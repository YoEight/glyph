pub mod params;

use crate::history::{file_backed_history, in_memory_history, History};
use crate::persistence::{FileBackend, Noop, Persistence};
use crate::Params;
use clap::Parser;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::{
    cursor::{self, MoveTo, MoveToNextLine},
    event, queue,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::fmt::Display;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Options {
    prompt: String,
    header: String,
    author: String,
    version: String,
    date: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            prompt: "λ".to_string(),
            header: Default::default(),
            author: Default::default(),
            version: Default::default(),
            date: Default::default(),
        }
    }
}

impl Options {
    pub fn prompt(self, prompt: impl AsRef<str>) -> Self {
        Self {
            prompt: prompt.as_ref().to_string(),
            ..self
        }
    }

    pub fn header(self, header: impl AsRef<str>) -> Self {
        Self {
            header: header.as_ref().to_string(),
            ..self
        }
    }

    pub fn author(self, author: impl AsRef<str>) -> Self {
        Self {
            author: author.as_ref().to_string(),
            ..self
        }
    }

    pub fn version(self, version: impl AsRef<str>) -> Self {
        Self {
            version: version.as_ref().to_string(),
            ..self
        }
    }

    pub fn date(self, date: impl AsRef<str>) -> Self {
        Self {
            date: date.as_ref().to_string(),
            ..self
        }
    }
}

#[derive(Debug)]
pub enum Input<C> {
    String(String),
    Exit,
    Command(C),
}

impl<A> Input<A> {
    pub fn map<F, B>(self, fun: F) -> Input<B>
    where
        F: FnOnce(A) -> B,
    {
        match self {
            Input::String(s) => Input::String(s),
            Input::Exit => Input::Exit,
            Input::Command(a) => Input::Command(fun(a)),
        }
    }

    pub fn flat_map<F, B>(self, fun: F) -> Input<B>
    where
        F: FnOnce(A) -> Input<B>,
    {
        match self {
            Input::String(s) => Input::String(s),
            Input::Exit => Input::Exit,
            Input::Command(a) => fun(a),
        }
    }
}

pub struct Inputs<A> {
    options: Options,
    terminated: bool,
    buffer: String,
    offset: u16,
    start_pos: u16,
    history: History<A>,
    inflight_buffer: Option<String>,
}

pub fn in_memory_inputs(options: Options) -> io::Result<Inputs<Noop>> {
    Inputs::new(options, in_memory_history()?)
}

pub fn file_backed_inputs(
    options: Options,
    path: impl AsRef<Path>,
) -> io::Result<Inputs<FileBackend>> {
    Inputs::new(options, file_backed_history(path)?)
}

impl<A> Inputs<A>
where
    A: Persistence,
{
    pub fn new(options: Options, history: History<A>) -> io::Result<Inputs<A>> {
        let start_pos = options.prompt.chars().count() as u16 + 2;
        let mut padding = false;

        if !options.header.is_empty() {
            println!("{}", options.header);
            padding = true;
        }

        if !options.author.is_empty() {
            println!("Author: {}", options.author);
            padding = true;
        }

        if !options.version.is_empty() {
            println!("Version: {}", options.version);
            padding = true;
        }

        if !options.date.is_empty() {
            println!("Date: {}", options.date);
            padding = true;
        }

        if padding {
            println!();
        }

        Ok(Inputs {
            options,
            terminated: false,
            buffer: String::new(),
            offset: 0,
            start_pos,
            history,
            inflight_buffer: None,
        })
    }

    pub fn next_input(&mut self) -> io::Result<Option<Input<Params>>> {
        self.next_input_with(|args| Ok::<_, String>(Params::new(args)))
    }

    pub fn next_input_with_parser<P: Parser>(&mut self) -> io::Result<Option<Input<P>>> {
        self.next_input_with(|args| {
            let mut updated = vec![":".to_string()];

            updated.extend(args);
            P::try_parse_from(updated)
        })
    }

    pub fn next_input_with<F, E, C>(&mut self, parser: F) -> io::Result<Option<Input<C>>>
    where
        E: Display,
        F: Fn(Vec<String>) -> Result<C, E>,
    {
        if self.terminated {
            return Ok(None);
        }

        enable_raw_mode()?;
        let mut stdout = io::stdout();

        let (_, y) = cursor::position()?;

        queue!(stdout, MoveTo(0, y + 1))?;
        write!(stdout, "{} ", self.options.prompt)?;

        stdout.flush()?;

        loop {
            let c = event::read()?;
            let (_, y) = cursor::position()?;

            if let Event::Key(KeyEvent { code, modifiers }) = c {
                match code {
                    KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
                        self.offset = 0;
                        queue!(stdout, MoveTo(2, y))?;
                    }

                    KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
                        self.offset = self.buffer.len() as u16;
                        queue!(stdout, MoveTo(2 + self.offset, y))?;
                    }

                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        queue!(stdout, MoveTo(0, y))?;
                        println!();
                        self.terminated = true;
                        disable_raw_mode()?;
                        return Ok(Some(Input::Exit));
                    }

                    KeyCode::Backspace if self.offset > 0 => {
                        self.offset -= 1;
                        self.buffer.remove(self.offset as usize);
                        queue!(stdout, MoveTo(0, y), Clear(ClearType::CurrentLine))?;
                        write!(stdout, "{} {}", self.options.prompt, self.buffer)?;
                        queue!(stdout, MoveTo(self.start_pos + self.offset - 1, y))?;

                        if self.buffer.is_empty() {
                            self.inflight_buffer = None;
                        } else {
                            self.inflight_buffer = Some(self.buffer.clone());
                        }
                    }

                    KeyCode::Left if self.offset > 0 => {
                        self.offset -= 1;
                        queue!(stdout, MoveTo(0, y), Clear(ClearType::CurrentLine))?;
                        write!(stdout, "{} {}", self.options.prompt, self.buffer)?;
                        queue!(stdout, MoveTo(self.start_pos + self.offset - 1, y))?;
                    }

                    KeyCode::Right if self.offset < self.buffer.len() as u16 => {
                        self.offset += 1;
                        queue!(stdout, MoveTo(0, y), Clear(ClearType::CurrentLine))?;
                        write!(stdout, "{} {}", self.options.prompt, self.buffer)?;
                        queue!(stdout, MoveTo(self.start_pos + self.offset - 1, y))?;
                    }

                    KeyCode::Up => {
                        if let Some(entry) = self.history.prev_entry() {
                            self.offset = entry.len() as u16;
                            self.buffer = entry;

                            queue!(stdout, MoveTo(0, y), Clear(ClearType::CurrentLine))?;
                            write!(stdout, "{} {}", self.options.prompt, self.buffer,)?;
                            queue!(stdout, MoveTo(self.start_pos + self.offset - 1, y))?;
                        }
                    }

                    KeyCode::Down => {
                        if let Some(entry) = self
                            .history
                            .next_entry()
                            .or_else(|| self.inflight_buffer.clone())
                            .or_else(|| Some("".to_string()))
                        {
                            self.offset = entry.len() as u16;
                            self.buffer = entry;

                            queue!(stdout, MoveTo(0, y), Clear(ClearType::CurrentLine))?;
                            write!(stdout, "{} {}", self.options.prompt, self.buffer)?;
                            queue!(stdout, MoveTo(self.start_pos + self.offset - 1, y))?;
                        }
                    }

                    KeyCode::Enter => {
                        let line = std::mem::replace(&mut self.buffer, String::new());
                        let line = line.as_str().trim();

                        if line.is_empty() {
                            queue!(stdout, MoveToNextLine(1))?;
                            write!(stdout, "{} ", self.options.prompt)?;

                            stdout.flush()?;
                            continue;
                        }

                        self.history.push(line.to_string())?;
                        self.offset = 0;

                        if let Some(cmd) = line.strip_prefix(":") {
                            if cmd.is_empty() {
                                continue;
                            }

                            let params = cmd
                                .split_whitespace()
                                .map(|c| c.to_string())
                                .collect::<Vec<_>>();

                            match parser(params) {
                                Err(e) => {
                                    stdout.flush()?;
                                    disable_raw_mode()?;
                                    println!();
                                    println!("{}", e);
                                    enable_raw_mode()?;
                                    queue!(stdout, MoveTo(0, y + 1))?;
                                    write!(stdout, "{} ", self.options.prompt)?;
                                    stdout.flush()?;

                                    continue;
                                }

                                Ok(c) => {
                                    queue!(stdout, MoveToNextLine(1))?;
                                    stdout.flush()?;

                                    self.inflight_buffer = None;

                                    disable_raw_mode()?;
                                    println!();
                                    return Ok(Some(Input::Command(c)));
                                }
                            }
                        }

                        queue!(stdout, MoveToNextLine(1))?;
                        stdout.flush()?;

                        self.inflight_buffer = None;

                        disable_raw_mode()?;
                        println!();

                        return Ok(Some(Input::String(line.to_string())));
                    }

                    KeyCode::Char(c) => {
                        self.offset += 1;

                        if self.offset < (self.buffer.len() + 1) as u16 {
                            self.buffer.insert((self.offset as usize) - 1, c);
                            queue!(stdout, MoveTo(0, y), Clear(ClearType::CurrentLine))?;
                            write!(stdout, "{} {}", self.options.prompt, self.buffer)?;
                            queue!(stdout, MoveTo(self.start_pos + self.offset - 1, y))?;
                        } else {
                            self.buffer.push(c);

                            queue!(stdout, MoveTo(0, y), Clear(ClearType::CurrentLine))?;
                            write!(stdout, "{} {}", self.options.prompt, self.buffer,)?;
                            queue!(stdout, MoveTo(self.start_pos + self.offset - 1, y),)?;
                        }

                        self.inflight_buffer = Some(self.buffer.clone());
                    }
                    _ => {}
                }
            }

            stdout.flush()?;
        }
    }
}
