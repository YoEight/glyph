use crate::history::{file_backed_history, in_memory_history, History};
use crate::persistence::{FileBackend, Noop, Persistence};
use std::io::{self, Stdin, Write};
use std::path::Path;
use termion::cursor::DetectCursorPos;
use termion::event::Key;
use termion::input::{Keys, TermRead};
use termion::raw::IntoRawMode;

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
    keys: Keys<Stdin>,
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
        let keys = io::stdin().keys();
        let start_pos = options.prompt.chars().count() as u16 + 2;

        Ok(Inputs {
            options,
            keys,
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

        let mut stdout = io::stdout().into_raw_mode()?;

        let (_, y) = stdout.cursor_pos()?;

        write!(
            stdout,
            "{}{} ",
            termion::cursor::Goto(1, y + 1),
            self.options.prompt
        )?;

        stdout.flush()?;

        while let Some(c) = self.keys.next().transpose()? {
            let (_, y) = stdout.cursor_pos()?;

            match c {
                Key::Ctrl('c') => {
                    println!();
                    self.terminated = true;
                    return Ok(Some(Input::Exit));
                }

                Key::Backspace if self.offset > 0 => {
                    self.offset -= 1;
                    self.buffer.remove(self.offset as usize);
                    write!(
                        stdout,
                        "{}{}{} {}{}",
                        termion::cursor::Goto(1, y),
                        termion::clear::CurrentLine,
                        self.options.prompt,
                        self.buffer,
                        termion::cursor::Goto(self.start_pos + self.offset, y)
                    )?;

                    if self.buffer.is_empty() {
                        self.inflight_buffer = None;
                    } else {
                        self.inflight_buffer = Some(self.buffer.clone());
                    }
                }

                Key::Left if self.offset > 0 => {
                    self.offset -= 1;
                    write!(
                        stdout,
                        "{}{}{} {}{}",
                        termion::cursor::Goto(1, y),
                        termion::clear::CurrentLine,
                        self.options.prompt,
                        self.buffer,
                        termion::cursor::Goto(self.start_pos + self.offset, y)
                    )?;
                }

                Key::Right if self.offset < self.buffer.len() as u16 => {
                    self.offset += 1;
                    write!(
                        stdout,
                        "{}{}{} {}{}",
                        termion::cursor::Goto(1, y),
                        termion::clear::CurrentLine,
                        self.options.prompt,
                        self.buffer,
                        termion::cursor::Goto(self.start_pos + self.offset, y)
                    )?;
                }

                Key::Up => {
                    if let Some(entry) = self.history.prev_entry() {
                        self.offset = entry.len() as u16;
                        self.buffer = entry;
                        write!(
                            stdout,
                            "{}{}{} {}",
                            termion::cursor::Goto(1, y),
                            termion::clear::CurrentLine,
                            self.options.prompt,
                            self.buffer
                        )?;
                    }
                }

                Key::Down => {
                    if let Some(entry) = self
                        .history
                        .next_entry()
                        .or_else(|| self.inflight_buffer.clone())
                        .or_else(|| Some("".to_string()))
                    {
                        self.offset = entry.len() as u16;
                        self.buffer = entry;
                        write!(
                            stdout,
                            "{}{}{} {}",
                            termion::cursor::Goto(1, y),
                            termion::clear::CurrentLine,
                            self.options.prompt,
                            self.buffer
                        )?;
                    }
                }

                Key::Char('\n') => {
                    let line = std::mem::replace(&mut self.buffer, String::new());
                    let line = line.as_str().trim();

                    if line.is_empty() {
                        write!(
                            stdout,
                            "\n{}{} ",
                            termion::cursor::Goto(1, y + 1),
                            self.options.prompt
                        )?;
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

                        write!(stdout, "\n{}", termion::cursor::Goto(1, y + 1))?;
                        stdout.flush()?;

                        self.inflight_buffer = None;

                        return Ok(Some(Input::Command { name, params }));
                    }

                    write!(stdout, "\n{}", termion::cursor::Goto(1, y + 1))?;
                    stdout.flush()?;

                    self.inflight_buffer = None;

                    return Ok(Some(Input::String(line.to_string())));
                }

                Key::Char(c) => {
                    self.offset += 1;

                    if self.offset < (self.buffer.len() + 1) as u16 {
                        self.buffer.insert((self.offset as usize) - 1, c);
                        write!(
                            stdout,
                            "{}{}{} {}{}",
                            termion::cursor::Goto(1, y),
                            termion::clear::CurrentLine,
                            self.options.prompt,
                            self.buffer,
                            termion::cursor::Goto(self.start_pos + self.offset, y)
                        )?;
                    } else {
                        self.buffer.push(c);
                        write!(
                            stdout,
                            "{}{}{} {}",
                            termion::cursor::Goto(1, y),
                            termion::clear::CurrentLine,
                            self.options.prompt,
                            self.buffer
                        )?;
                    }

                    self.inflight_buffer = Some(self.buffer.clone());
                }
                _ => {}
            }
            stdout.flush()?;
        }

        self.terminated = true;
        Ok(Some(Input::Exit))
    }
}
