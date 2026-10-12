//! Host tests of `console` (issue u004) and of how a program's output reaches it (libmind/src/output.rs, issue 162):
//! the message format; output cut anywhere, wrapped, scrolled; the command line, its history and the commands.
#![allow(dead_code)]
extern crate alloc;
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/keys.rs"]
mod keys;
#[path = "../libmind/src/util.rs"]
mod util;
#[path = "../libmind/src/tui/mod.rs"]
mod tui;
#[path = "../libmind/src/output.rs"]
mod output;
#[path = "../console/src/screen.rs"]
mod screen;
// libmind's help keys (000-KRN-0066) without its system calls; `help_keys_are_libminds` holds them to the source.
mod process {
    pub const HELP_KEYS: [&str; 7] = ["--help", "-help", "-h", "/help", "/h", "/?", "-?"];
    pub fn asks_help(args: &str) -> bool { HELP_KEYS.iter().any(|key| args.trim().eq_ignore_ascii_case(key)) }
}

use abi::*;
use keys::{event, Key};
use screen::{parse, Command, Kind, Screen};
use tui::{Cell, Grid};

fn chr(ch: char) -> Key { Key(event(0, ch as u32, 0)) }
fn code(code: u16) -> Key { Key(event(code, 0, 0)) }
fn typed(screen: &mut Screen, text: &str) -> Option<String> {
    for ch in text.chars() { screen.key(chr(ch), 10); }
    screen.key(code(KEY_ENTER), 10)
}
fn draw(screen: &mut Screen, cols: usize, rows: usize, busy: &str) -> (Vec<String>, Option<(usize, usize)>) {
    let mut cells = vec![Cell::BLANK; cols * rows];
    let mut grid = Grid::new(&mut cells, cols, rows);
    let cursor = screen.draw(&mut grid, busy);
    ((0..rows).map(|y| (0..cols).map(|x| grid.get(x, y).ch).collect::<String>().trim_end().to_string()).collect(), cursor)
}

#[test]
fn output_messages() {
    for text in ["", "a", "up 0:00:01, load 0.04", "Привет, мир!!!", "exactly fifteen"] {
        let bytes = text.as_bytes();
        let mut out = [0u8; output::CHUNK];
        for chunk in bytes.chunks(output::CHUNK) {
            let len = output::unpack(output::pack(chunk), &mut out);
            assert_eq!(&out[..len], chunk);
        }
    }
    assert_eq!(output::unpack(output::pack(&[7u8; 40]), &mut [0; output::CHUNK]), output::CHUNK, "at most 15 bytes a message");
    assert_eq!(output::unpack([0xFF, 0], &mut [0; output::CHUNK]), output::CHUNK, "a length beyond 15 is cut");
}

#[test]
fn what_programs_print() {
    let mut screen = Screen::new();
    // Cut anywhere, a Cyrillic character included; the unfinished line shows last.
    let text = "строка один\nline\ttwo\r\nпоследняя";
    for chunk in text.as_bytes().chunks(output::CHUNK) { screen.output(chunk); }
    let (rows, _) = draw(&mut screen, 40, 6, "");
    assert_eq!(&rows[..3], ["строка один", "line    two", "последняя"]);
    assert_eq!(rows[5], ">");
    // What console says ends a program's unfinished line; an invalid byte shows as ?.
    screen.say("uptime: no such program", Kind::Error);
    screen.output(&[b'x', 0xFF, b'y', b'\n']);
    let (rows, _) = draw(&mut screen, 40, 8, "");
    assert_eq!(&rows[..5], ["строка один", "line    two", "последняя", "uptime: no such program", "x?y"]);
    assert_eq!(screen.lines[3].1, Kind::Error);
    // Words stay whole where a space is near the end of a row.
    screen.clear();
    screen.say("type it in the shell, as before", Kind::Note);
    assert_eq!(screen.rows(12).into_iter().map(|(t, _)| t).collect::<Vec<_>>(), ["type it in", "the shell,", "as before"]);
    assert_eq!(screen.rows(4).into_iter().map(|(t, _)| t).collect::<Vec<_>>(), ["type", "it", "in", "the", "shel", "l,", "as", "befo", "re"]);
    // Long lines wrap; the newest rows stay at the bottom.
    screen.clear();
    for n in 0..30 { screen.output(format!("line {}\n", n).as_bytes()); }
    screen.output("x".repeat(25).as_bytes());
    let (rows, _) = draw(&mut screen, 10, 6, "");
    assert_eq!(&rows[..5], ["line 28", "line 29", "xxxxxxxxxx", "xxxxxxxxxx", "xxxxx"]);
    // PgUp and the wheel scroll back (a note says how far), no further than the first line; typing comes back down.
    screen.key(code(KEY_PAGE_UP), 4);
    let (rows, _) = draw(&mut screen, 30, 6, "");
    assert!(rows[0].starts_with("line 22") && rows[0].ends_with("↑ 4 more below"), "{:?}", rows);
    assert_eq!(rows[4], "line 26");
    screen.wheel(1);
    let (rows, _) = draw(&mut screen, 30, 6, "");
    assert_eq!(rows[4], "line 29");
    screen.wheel(-100);
    let (rows, _) = draw(&mut screen, 30, 6, "");
    assert!(rows[0].starts_with("line 0 ") && rows[4] == "line 4", "{:?}", rows);
    screen.key(chr('a'), 4);
    let (rows, _) = draw(&mut screen, 30, 6, "");
    assert_eq!(rows[4], "x".repeat(25));
}

#[test]
fn the_command_line() {
    let mut screen = Screen::new();
    assert_eq!(typed(&mut screen, "uptime"), Some("uptime".into()));
    assert_eq!(typed(&mut screen, "grep -i x docs/notes.txt"), Some("grep -i x docs/notes.txt".into()));
    assert_eq!(screen.lines.iter().map(|(t, k)| (t.as_str(), *k)).collect::<Vec<_>>(), [("> uptime", Kind::Command), ("> grep -i x docs/notes.txt", Kind::Command)]);
    // ↑ ↓ recall earlier lines; Esc clears the line.
    screen.key(code(KEY_UP), 10);
    assert_eq!(screen.line.as_str(), "grep -i x docs/notes.txt");
    screen.key(code(KEY_UP), 10);
    assert_eq!(screen.line.as_str(), "uptime");
    screen.key(code(KEY_DOWN), 10);
    screen.key(code(KEY_DOWN), 10);
    assert_eq!(screen.line.as_str(), "");
    screen.key(code(KEY_UP), 10);
    screen.key(code(KEY_ESC), 10);
    assert_eq!(screen.line.as_str(), "");
    // The cursor after the prompt; what runs at the right.
    for ch in "df".chars() { screen.key(chr(ch), 10); }
    let (rows, cursor) = draw(&mut screen, 40, 5, "uptime");
    assert_eq!(cursor, Some((4, 4)));
    assert!(rows[4].starts_with("> df") && rows[4].ends_with("uptime"), "{}", rows[4]);
    // Ctrl+L clears.
    screen.key(Key(event(0, 'l' as u32, MOD_CTRL)), 10);
    assert!(screen.lines.is_empty());
    // The commands.
    assert_eq!(parse("  "), Command::Nothing);
    assert_eq!(parse("help"), Command::Help);
    assert_eq!(parse("clear"), Command::Clear);
    assert_eq!(parse("exit"), Command::Exit);
    assert_eq!(parse("list"), Command::List);
    assert_eq!(parse("uptime.elf"), Command::Run { name: "uptime", args: "" });
    assert_eq!(parse(" find  ram: -name *.txt "), Command::Run { name: "find", args: "ram: -name *.txt" });
    // console's own commands; the shell's; `run` for a program whose name a command has (issue u006).
    assert_eq!(parse("ps"), Command::Builtin { name: "ps", args: "" });
    assert_eq!(parse("ls docs"), Command::Builtin { name: "ls", args: "docs" });
    assert_eq!(parse("ping ya.ru"), Command::Builtin { name: "ping", args: "ya.ru" });
    assert_eq!(parse("run ping"), Command::Run { name: "ping", args: "" });
    assert_eq!(parse("run clock --text"), Command::Run { name: "clock", args: "--text" });
    assert_eq!(parse("run"), Command::Run { name: "", args: "" });
    assert_eq!(parse("kill 3"), Command::Shell("kill"));
    assert_eq!(parse("nslookup ya.ru"), Command::Shell("nslookup"));
    // To the shell through its commands (211-APP-0044): `date set` and `netpolicy` too; `date` alone stays console's.
    assert_eq!(parse("date set 2026-10-10 12:00"), Command::Shell("date"));
    assert_eq!(parse("date"), Command::Builtin { name: "date", args: "" });
    assert_eq!(parse("netpolicy add x"), Command::Shell("netpolicy"));
    assert!(screen::ASKED.iter().all(|c| screen::SHELL_ONLY.contains(c)));
}

#[test]
fn help_keys_are_libminds() {
    let source = include_str!("../libmind/src/process.rs");
    assert!(source.contains(&format!("pub const HELP_KEYS: [&str; 7] = {:?};", process::HELP_KEYS)));
    assert!(source.contains("pub fn asks_help(args: &str) -> bool { HELP_KEYS.iter().any(|key| args.trim().eq_ignore_ascii_case(key)) }"));
}

#[test]
fn a_help_key_after_a_command_says_what_it_does() {
    // 000-APP-0054: console's commands answer the keys every program answers, with their line; the shell's commands
    // with the shell's lines; a program prints its own text (console runs it with a help key as a console program).
    for (line, name) in [("date -h", "date"), ("date /?", "date"), ("ls --help", "ls"), ("ps -help", "ps"), ("ps -HELP", "ps"), ("write /H", "write"),
                         ("help ls", "ls"), ("help -h", "help"), ("exit -?", "exit"), ("run /help", "run"), ("kill -h", "kill"), ("help free", "free")] {
        assert_eq!(parse(line), Command::About(name), "{}", line);
    }
    assert_eq!(parse("fm /?"), Command::Run { name: "fm", args: "/?" });
    assert_eq!(parse("run fm -h"), Command::Run { name: "fm", args: "-h" });
    assert_eq!(parse("help fm"), Command::Run { name: "fm", args: "--help" });
    assert_eq!(parse("help"), Command::Help);
    // Not a help key alone: the command as before.
    assert_eq!(parse("ls -h docs"), Command::Builtin { name: "ls", args: "-h docs" });
    assert_eq!(parse("date set -h"), Command::Shell("date"));
    // Each of console's commands has its line, and the shell's commands are the shell's to explain.
    for name in screen::BUILTINS.iter().chain(["list", "clear", "cls", "exit", "quit", "run", "help", "?"].iter()) {
        let line = screen::line_of(name).unwrap_or_else(|| panic!("{}", name));
        assert!(line.starts_with(name) || line.contains(&format!(", {}", name)) || line.contains(&format!(" {} ", name)), "{}: {}", name, line);
    }
    assert!(screen::SHELL_ONLY.iter().all(|c| screen::line_of(c).is_none()));
}

#[test]
fn a_carriage_return_writes_the_line_again() {
    // Issue u016: `clock --line` writes its line again after \r; what comes next overwrites the old characters.
    let mut screen = Screen::new();
    screen.output(b"started\n\r12:00:00  2026-10-06");
    screen.output(b"\r12:00:01  2026-10-06");
    screen.output(b"\rab");
    let rows = |screen: &Screen| screen.rows(40).into_iter().map(|(t, _)| t).collect::<Vec<_>>();
    assert_eq!(rows(&screen), ["started", "ab:00:01  2026-10-06"]);
    // A line ended with CRLF stays as it is.
    screen.output(b"\r\nnext\r\n");
    assert_eq!(rows(&screen), ["started", "ab:00:01  2026-10-06", "next"]);
}
