use crate::history::{file_backed_history, in_memory_history, History};
use crate::persistence::{FileBackend, Noop, Persistence};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::{
    cursor::{self, MoveTo, MoveToNextLine},
    event, queue,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Options {
    prompt: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            prompt: "λ".to_string(),
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
}

#[derive(Debug)]
pub enum Input {
    String(String),
    Exit,
    Command { name: String, params: Vec<String> },
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

    pub fn next_input(&mut self) -> io::Result<Option<Input>> {
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
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        write!(stdout, "\n")?;
                        stdout.flush()?;
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

                            let mut params = cmd
                                .split_whitespace()
                                .map(|s| s.to_string())
                                .collect::<Vec<String>>();

                            let name = params.remove(0);

                            queue!(stdout, MoveToNextLine(1))?;
                            stdout.flush()?;

                            self.inflight_buffer = None;

                            disable_raw_mode()?;
                            println!();

                            return Ok(Some(Input::Command { name, params }));
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
