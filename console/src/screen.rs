//! The text of `console` (issue u004): what its programs printed and what it says itself, in lines, with the command
//! line under them; wrapped to the grid, scrolled back with PgUp/PgDn or the mouse wheel; ↑ ↓ recall earlier commands.
//! No system calls: tests/console_host.rs.
use crate::keys::{Code, Key};
use crate::tui::widgets::{Edit, InputLine};
use crate::tui::{Grid, Style};
use alloc::string::String;
use alloc::vec::Vec;

pub const LINES_MAX: usize = 2000;
pub const HISTORY: usize = 32;
pub const PROMPT: &str = "> ";

/// Whose a line is: a program's output, a command typed, what `console` says, a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Output, Command, Note, Error }

impl Kind {
    pub fn style(self) -> Style {
        match self {
            Kind::Output => Style::new(0xD0D0D0, BACKGROUND), Kind::Command => Style::new(0xFFFFFF, BACKGROUND),
            Kind::Note => Style::new(0x70A0C0, BACKGROUND), Kind::Error => Style::new(0xF07070, BACKGROUND),
        }
    }
}
pub const BACKGROUND: u32 = 0x101418;

/// What a line typed asks for: console's own commands; a command of the shell, which console cannot do (it holds
/// no process control and no network of the shell's); a program (`run` starts a program even where a command has
/// its name, as `run ping` the IPC demo).
/// `About`: what one of console's commands or the shell's does (`help <command>`, `<command> -h`; 000-APP-0054).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command<'a> { Nothing, Help, Clear, Exit, List, Builtin { name: &'a str, args: &'a str }, Shell(&'a str), Run { name: &'a str, args: &'a str }, About(&'a str) }

/// Commands console does itself, with what it holds (issue u006).
pub const BUILTINS: [&str; 10] = ["ps", "ls", "cat", "date", "time", "ping", "mkdir", "rm", "mv", "write"];
/// The shell's commands that need what only the shell holds: process control, its network and device clients. With
/// the shell's commands (`SLOT_SHELL`, 211-APP-0044) console sends them to the shell; `fg`, `boot`, `keymap` and
/// `screenshot` act on the shell's own screen, and the shell refuses them.
pub const SHELL_ONLY: [&str; 32] = ["kill", "fg", "logs", "stop", "boot", "ip", "nslookup", "fetch", "https", "tls", "net", "netgrants", "netrevoke",
    "netpolicy", "tpm", "pmap", "stat", "free", "cpus", "physmap", "irqs", "devices", "endpoints", "faults", "quotas", "budget", "heap", "sync", "logger",
    "reboot", "keymap", "screenshot"];
/// Of those, the ones the shell runs in its own window after the user agrees there (shell/src/clients.rs).
pub const ASKED: [&str; 14] = ["kill", "stop", "reboot", "budget", "netrevoke", "netpolicy", "logs", "stat", "pmap", "logger", "nslookup", "fetch", "https", "tpm"];
/// Console's own commands, a line each, for `help <command>` and `<command> <help key>` (000-APP-0054).
pub const LINES: [(&[&str], &str); 12] = [
    (&["ps"], "ps: the tasks, from sysmon"),
    (&["ls"], "ls [dir]: a directory's entries and sizes (ram:, log:, data/, docs/, …)"),
    (&["cat"], "cat <file>: a file's text, up to 16 KiB"),
    (&["date"], "date: the date and time from the RTC; date set YYYY-MM-DD HH:MM[:SS]: the shell sets the clock after you agree in its window"),
    (&["time"], "time: the time of day and the uptime"),
    (&["ping"], "ping <host>: the network ping, on console's flow grant (run ping: the IPC demo)"),
    (&["mkdir", "rm", "mv", "write"], "mkdir <dir>, rm <path>, mv <from> <to>, write <file> <text>: change files on ram:, on log: and in data/"),
    (&["list"], "list: the programs on the boot disk"),
    (&["clear", "cls"], "clear, cls (Ctrl+L): clear the screen"),
    (&["exit", "quit"], "exit, quit: close console"),
    (&["run"], "run <program> [arguments]: start a program even where a command has its name"),
    (&["help", "?"], "help, ? [command or program]: console's help; with a name, what it does (as <name> -h, /? or --help)"),
];

/// Console's line for its command `name`.
pub fn line_of(name: &str) -> Option<&'static str> { LINES.iter().find(|(names, _)| names.contains(&name)).map(|(_, line)| *line) }

// Whether `name` is a command, console's or the shell's, rather than a program.
fn is_command(name: &str) -> bool { line_of(name).is_some() || SHELL_ONLY.contains(&name) }

// The first word and the rest.
fn split(line: &str) -> (&str, &str) { line.split_once(char::is_whitespace).map_or((line, ""), |(n, a)| (n, a.trim())) }
// A program's name without `.elf`.
fn program(name: &str) -> &str { name.strip_suffix(".elf").unwrap_or(name) }

pub fn parse(line: &str) -> Command<'_> {
    let (name, args) = split(line.trim());
    match name {
        "" => Command::Nothing,
        // A help key after a command: its line; after a program, the program prints its own text (`run`).
        _ if crate::process::asks_help(args) && is_command(name) => Command::About(name),
        "help" | "?" if args.is_empty() => Command::Help,
        "help" | "?" => { let (name, _) = split(args); if is_command(name) { Command::About(name) } else { Command::Run { name: program(name), args: "--help" } } }
        "clear" | "cls" => Command::Clear,
        "exit" | "quit" => Command::Exit,
        "list" if args.is_empty() => Command::List,
        "run" => { let (name, args) = split(args); Command::Run { name: program(name), args } }
        "date" if args.starts_with("set") => Command::Shell(name),
        _ if BUILTINS.contains(&name) => Command::Builtin { name, args },
        _ if SHELL_ONLY.contains(&name) => Command::Shell(name),
        _ => Command::Run { name: program(name), args },
    }
}

pub struct Screen {
    pub lines: Vec<(String, Kind)>,
    /// A program's output since its last newline, shown as the last line.
    partial: String,
    at: usize, // where the next character of `partial` goes, in characters: before its end after a \r (issue u016)
    /// The start of a character a message split.
    carry: Vec<u8>,
    /// Rows scrolled back from the end.
    pub scroll: usize,
    pub line: InputLine,
    history: Vec<String>,
    recall: Option<usize>,
}

impl Default for Screen { fn default() -> Self { Self::new() } }

impl Screen {
    pub fn new() -> Self { Self { lines: Vec::new(), partial: String::new(), at: 0, carry: Vec::new(), scroll: 0, line: InputLine::new(), history: Vec::new(), recall: None } }

    fn push(&mut self, text: String, kind: Kind) {
        if self.lines.len() == LINES_MAX { self.lines.remove(0); }
        self.lines.push((text, kind));
    }

    /// Bytes a program printed (UTF-8, perhaps cut in the middle of a character).
    pub fn output(&mut self, bytes: &[u8]) {
        self.carry.extend_from_slice(bytes);
        let carry = core::mem::take(&mut self.carry);
        let mut rest = &carry[..];
        loop {
            match core::str::from_utf8(rest) {
                Ok(text) => { self.chars(text); break; }
                Err(error) => {
                    let (good, after) = rest.split_at(error.valid_up_to());
                    self.chars(core::str::from_utf8(good).unwrap_or(""));
                    match error.error_len() {
                        None => { self.carry = after.to_vec(); break; } // the rest of the character comes next
                        Some(bad) => { self.chars("?"); rest = &after[bad..]; }
                    }
                }
            }
        }
    }

    fn chars(&mut self, text: &str) {
        for ch in text.chars() {
            match ch {
                '\n' => { let line = core::mem::take(&mut self.partial); self.at = 0; self.push(line, Kind::Output); }
                // The start of the line again: what comes next overwrites it (`clock --line`, issue u016).
                '\r' => self.at = 0,
                '\t' => { let next = (self.at / 8 + 1) * 8; while self.at < next { self.put(' '); } }
                c if (c as u32) < 0x20 || c == '\u{7F}' => {} // other controls
                c => self.put(c),
            }
        }
    }

    // A character at `at`: over the one there, or at the end.
    fn put(&mut self, c: char) {
        match self.partial.char_indices().nth(self.at) {
            Some((i, old)) => self.partial.replace_range(i..i + old.len_utf8(), c.encode_utf8(&mut [0; 4])),
            None => self.partial.push(c),
        }
        self.at += 1;
    }

    /// A line of `console`'s own; a program's unfinished line ends first.
    pub fn say(&mut self, text: &str, kind: Kind) {
        if !self.partial.is_empty() { let line = core::mem::take(&mut self.partial); self.at = 0; self.push(line, Kind::Output); }
        for line in text.split('\n') { self.push(String::from(line), kind); }
    }

    pub fn clear(&mut self) { self.lines.clear(); self.partial.clear(); self.at = 0; self.scroll = 0; }

    /// A key; Enter gives the line typed (it goes into the history and is shown after the prompt). `page`: the rows
    /// PgUp and PgDn scroll.
    pub fn key(&mut self, key: Key, page: usize) -> Option<String> {
        match key.code() {
            Code::Up | Code::Down => {
                if self.history.is_empty() { return None; }
                let last = self.history.len() - 1;
                self.recall = match (self.recall, key.code()) {
                    (None, Code::Up) => Some(last), (None, _) => None,
                    (Some(i), Code::Up) => Some(i.saturating_sub(1)), (Some(i), _) if i < last => Some(i + 1), _ => None,
                };
                match self.recall { Some(i) => { let text = self.history[i].clone(); self.line.set(&text); } None => self.line.clear() }
                None
            }
            Code::PageUp => { self.scroll += page.max(1); None }
            Code::PageDown => { self.scroll = self.scroll.saturating_sub(page.max(1)); None }
            Code::Esc => { self.line.clear(); self.recall = None; None }
            _ if key.is_ctrl('l') => { self.clear(); None }
            _ => match self.line.key(key) {
                Edit::Submit => {
                    let text = String::from(self.line.as_str());
                    self.line.clear();
                    self.recall = None;
                    self.scroll = 0;
                    self.say(&alloc::format!("{}{}", PROMPT, text), Kind::Command);
                    if !text.trim().is_empty() && self.history.last() != Some(&text) {
                        if self.history.len() == HISTORY { self.history.remove(0); }
                        self.history.push(text.clone());
                    }
                    Some(text)
                }
                _ => { self.scroll = 0; None }
            },
        }
    }

    /// The wheel turned `steps` (negative: up): three rows a step.
    pub fn wheel(&mut self, steps: i32) {
        if steps < 0 { self.scroll += 3 * steps.unsigned_abs() as usize; } else { self.scroll = self.scroll.saturating_sub(3 * steps as usize); }
    }

    /// Every line cut to `width` columns (at a space where one is near the end), the program's unfinished line last.
    pub fn rows(&self, width: usize) -> Vec<(String, Kind)> {
        let width = width.max(1);
        let mut rows = Vec::new();
        for (text, kind) in self.lines.iter().map(|(t, k)| (t.as_str(), *k)).chain((!self.partial.is_empty()).then_some((self.partial.as_str(), Kind::Output))) {
            let chars: Vec<char> = text.chars().collect();
            if chars.is_empty() { rows.push((String::new(), kind)); continue; }
            // At the last space that fits, when there is one in the row's second half; else where the row is full.
            let mut start = 0;
            while start < chars.len() {
                let mut end = (start + width).min(chars.len());
                if end < chars.len() && chars[end] != ' ' {
                    if let Some(space) = chars[start + width / 2..end].iter().rposition(|&c| c == ' ') { end = start + width / 2 + space + 1; }
                }
                rows.push((chars[start..end].iter().collect::<String>().trim_end().into(), kind));
                start = end;
                while start < chars.len() && chars[start] == ' ' { start += 1; } // a row continued does not start with a space
            }
        }
        rows
    }

    /// Draws the lines above the command line; returns the cursor. `busy`: what runs, shown at the right of the
    /// command line.
    pub fn draw(&mut self, grid: &mut Grid, busy: &str) -> Option<(usize, usize)> {
        let (w, h) = (grid.cols, grid.rows);
        grid.clear(Kind::Output.style());
        if h == 0 { return None; }
        let rows = self.rows(w);
        let view = h - 1;
        self.scroll = self.scroll.min(rows.len().saturating_sub(view));
        let end = rows.len() - self.scroll;
        for (y, (text, kind)) in rows[end.saturating_sub(view)..end].iter().enumerate() { grid.text_max(0, y, text, w, kind.style()); }
        if self.scroll > 0 { grid.text_right(w, 0, &alloc::format!(" ↑ {} more below ", self.scroll), Kind::Note.style()); }
        let y = h - 1;
        let note = if busy.is_empty() { String::new() } else { alloc::format!(" {} ", busy) };
        let room = w.saturating_sub(note.chars().count());
        grid.text_max(0, y, PROMPT, room, Kind::Command.style());
        let column = self.line.draw(grid, PROMPT.len().min(room), y, room.saturating_sub(PROMPT.len()), Kind::Command.style());
        if !note.is_empty() { grid.text_right(w, y, &note, Kind::Note.style()); }
        Some((column, y))
    }
}
