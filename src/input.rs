use crate::history::History;
use std::io::{self, Stdin, Write};
use termion::cursor::DetectCursorPos;
use termion::event::Key;
use termion::input::{Keys, TermRead};
use termion::raw::IntoRawMode;

pub enum Input {
    String(String),
    Exit,
    Error(io::Error),
    Command { name: String, params: Vec<String> },
}

pub struct Inputs {
    terminated: bool,
    buffer: String,
    offset: u16,
    start_pos: u16,
    keys: Keys<Stdin>,
    history: History,
}

impl Inputs {
    pub fn new() -> Self {
        let keys = io::stdin().keys();

        Inputs {
            keys,
            terminated: false,
            buffer: String::new(),
            offset: 0,
            start_pos: 3,
            history: History::new(),
        }
    }
}

impl Iterator for Inputs {
    type Item = Input;

    fn next(&mut self) -> Option<Self::Item> {
        macro_rules! input_try {
            ( $( $x:expr)* ) => {
                $(
                    match $x {
                        Ok(value) => value,
                        Err(e) => {
                            self.terminated = true;
                            return Some(Input::Error(e));
                        }
                    }
                )*
            }
        }

        if self.terminated {
            return None;
        }

        let mut stdout = input_try!(io::stdout().into_raw_mode());

        let (_, y) = input_try!(stdout.cursor_pos());

        input_try!(write!(stdout, "\n{}λ ", termion::cursor::Goto(1, y + 1)));
        input_try!(stdout.flush());

        while let Some(c) = input_try!(self.keys.next().transpose()) {
            let (_, y) = input_try!(stdout.cursor_pos());

            match c {
                Key::Ctrl('c') => {
                    println!();
                    self.terminated = true;
                    return Some(Input::Exit);
                }

                Key::Backspace if self.offset > 0 => {
                    self.offset -= 1;
                    self.buffer.remove(self.offset as usize);
                    input_try!(write!(
                        stdout,
                        "{}{}λ {}{}",
                        termion::cursor::Goto(1, y),
                        termion::clear::CurrentLine,
                        self.buffer,
                        termion::cursor::Goto(self.start_pos + self.offset, y)
                    ));
                }

                Key::Left if self.offset > 0 => {
                    self.offset -= 1;
                    input_try!(write!(
                        stdout,
                        "{}{}λ {}{}",
                        termion::cursor::Goto(1, y),
                        termion::clear::CurrentLine,
                        self.buffer,
                        termion::cursor::Goto(self.start_pos + self.offset, y)
                    ));
                }

                Key::Right if self.offset < self.buffer.len() as u16 => {
                    self.offset += 1;
                    input_try!(write!(
                        stdout,
                        "{}{}λ {}{}",
                        termion::cursor::Goto(1, y),
                        termion::clear::CurrentLine,
                        self.buffer,
                        termion::cursor::Goto(self.start_pos + self.offset, y)
                    ));
                }

                Key::Up => {
                    if let Some(entry) = self.history.prev_entry() {
                        self.offset = entry.len() as u16;
                        self.buffer = entry;
                        input_try!(write!(
                            stdout,
                            "{}{}λ {}",
                            termion::cursor::Goto(1, y),
                            termion::clear::CurrentLine,
                            self.buffer
                        ));
                    }
                }

                Key::Down => {
                    if let Some(entry) = self.history.next_entry() {
                        self.offset = entry.len() as u16;
                        self.buffer = entry;
                        input_try!(write!(
                            stdout,
                            "{}{}λ {}",
                            termion::cursor::Goto(1, y),
                            termion::clear::CurrentLine,
                            self.buffer
                        ));
                    }
                }

                Key::Char('\n') => {
                    let line = std::mem::replace(&mut self.buffer, String::new());
                    let line = line.as_str().trim();

                    if line.is_empty() {
                        input_try!(write!(stdout, "\n{}λ ", termion::cursor::Goto(1, y + 1)));
                        input_try!(stdout.flush());
                        continue;
                    }

                    self.history.push(line.to_string());
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

                        return Some(Input::Command { name, params });
                    }

                    return Some(Input::String(line.to_string()));
                }

                Key::Char(c) => {
                    self.offset += 1;

                    if self.offset < (self.buffer.len() + 1) as u16 {
                        self.buffer.insert((self.offset as usize) - 1, c);
                        input_try!(write!(
                            stdout,
                            "{}{}λ {}{}",
                            termion::cursor::Goto(1, y),
                            termion::clear::CurrentLine,
                            self.buffer,
                            termion::cursor::Goto(self.start_pos + self.offset, y)
                        ));
                    } else {
                        self.buffer.push(c);
                        input_try!(write!(
                            stdout,
                            "{}{}λ {}",
                            termion::cursor::Goto(1, y),
                            termion::clear::CurrentLine,
                            self.buffer
                        ));
                    }
                }
                _ => {}
            }
            input_try!(stdout.flush());
        }

        self.terminated = true;
        Some(Input::Exit)
    }
}
