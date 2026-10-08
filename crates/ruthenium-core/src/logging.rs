use std::{
    borrow::Cow,
    io::{self, Write},
    sync::Arc,
};

use rustyline::{
    Editor, Helper, completion::Completer, highlight::Highlighter, hint::Hinter,
    history::FileHistory, validate::Validator,
};

use crate::server::Server;

pub struct ReadlineLogWrapper {
    readline: std::sync::Mutex<Option<Editor<RutheniumCommand, FileHistory>>>,
}

pub struct ConsoleWriter {
    printer: Option<Box<dyn rustyline::ExternalPrinter + Send>>,
    buffer: Vec<u8>,
}

impl ConsoleWriter {
    #[must_use]
    pub fn new(printer: Option<Box<dyn rustyline::ExternalPrinter + Send>>) -> Self {
        Self {
            printer,
            buffer: Vec::new(),
        }
    }
}

impl Write for ConsoleWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(ref mut printer) = self.printer {
            self.buffer.extend_from_slice(buf);
            while let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = self.buffer.drain(..=pos).collect();
                let msg = String::from_utf8_lossy(&line_bytes).into_owned();
                if printer.print(msg).is_err() {
                    let mut stdout = io::stdout().lock();
                    let _ = stdout.write_all(line_bytes.as_slice());
                    let _ = stdout.flush();
                }
            }
            Ok(buf.len())
        } else {
            io::stdout().write(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(ref mut printer) = self.printer {
            if !self.buffer.is_empty() {
                let buffer = std::mem::take(&mut self.buffer);
                let msg = String::from_utf8_lossy(&buffer).into_owned();
                if printer.print(msg).is_err() {
                    let mut stdout = io::stdout().lock();
                    let _ = stdout.write_all(buffer.as_slice());
                    let _ = stdout.flush();
                }
            }
            Ok(())
        } else {
            io::stdout().flush()
        }
    }
}

impl ReadlineLogWrapper {
    pub const fn new(rl: Option<Editor<RutheniumCommand, FileHistory>>) -> Self {
        Self {
            readline: std::sync::Mutex::new(rl),
        }
    }

    pub fn take_readline(&self) -> Option<Editor<RutheniumCommand, FileHistory>> {
        self.readline
            .lock()
            .map_or_else(|_| None, |mut guard| guard.take())
    }

    pub fn return_readline(&self, rl: Editor<RutheniumCommand, FileHistory>) {
        if let Ok(mut result) = self.readline.lock() {
            let _ = result.insert(rl);
        }
    }
}

#[derive(Clone, Default)]
pub struct RutheniumCommand {
    pub server: Arc<std::sync::RwLock<Option<Arc<Server>>>>,
    pub rt: Arc<std::sync::OnceLock<tokio::runtime::Handle>>,
}

impl RutheniumCommand {
    pub fn new() -> Self {
        Self {
            server: Arc::new(std::sync::RwLock::new(None)),
            rt: Arc::new(std::sync::OnceLock::new()),
        }
    }
}

impl Helper for RutheniumCommand {}
impl Highlighter for RutheniumCommand {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        line.find(' ').map_or_else(
            || Cow::Owned(format!("\x1b[1;36m{line}\x1b[0m")),
            |first_space| {
                let (cmd, args) = line.split_at(first_space);
                Cow::Owned(format!("\x1b[1;36m{cmd}\x1b[0m{args}"))
            },
        )
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[90m{hint}\x1b[0m"))
    }
}

impl Hinter for RutheniumCommand {
    type Hint = String;
    fn hint(&self, line: &str, pos: usize, ctx: &rustyline::Context<'_>) -> Option<Self::Hint> {
        if line.is_empty() || pos < line.len() {
            return None;
        }

        if let Ok((_, candidates)) = self.complete(line, pos, ctx)
            && let Some(first) = candidates.first()
        {
            let last_word = line.split_whitespace().last().unwrap_or("");
            if first.starts_with('<') {
                return line.ends_with(' ').then(|| first.clone());
            }

            if let Some(stripped) = first.strip_prefix(last_word) {
                return Some(stripped.to_string());
            }
        }
        None
    }
}

impl Validator for RutheniumCommand {}

impl Completer for RutheniumCommand {
    type Candidate = String;

    fn complete(
        &self, // FIXME should be `&mut self`
        _line: &str,
        _pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        Ok((0, Vec::new()))
    }
}
