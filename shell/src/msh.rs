// msh, the shell's script language (issue 094; the language is mind::script): `msh file [args]`, `msh -c "…"`,
// `msh --check file`, a name ending in .msh, and statements typed at the prompt (`let`, `if`, `for`, …).
//
// A script runs with the authority of the shell session, but no more than its `requires:` line declares (the words
// of `mind::request!`): the shell's commands that change files, use the network, write the log or end programs are
// refused without it, and programs it starts get only what it declared. A script from outside the boot disk asks the
// user once. What the user types at the prompt (and `msh -c`) has the session's authority.
use crate::Shell;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write;
use mind::control;
use mind::process::Exit;
use mind::script::{parse, Host, Interpreter, Script, Value};

/// The functions the shell gives scripts (beside mind::script's).
pub const HOST_FUNCTIONS: [&str; 8] = ["capture", "ps", "services", "files", "glob", "log", "sleep", "now"];
// `firmware` and `camera` still ask the user each time; declaring them only lets a program ask (211-APP-0013).
const WORDS: [&str; 17] = ["console", "sysinfo", "file", "files", "lifecycle", "log", "network", "authority", "window", "window-manager", "display", "gpio", "camera", "blockstore", "firmware", "tls", "parse"];
const MAX_SCRIPT: usize = 64 * 1024;

// The shell's commands that need a declared word in a script.
fn needs(command: &str) -> Option<&'static str> {
    let is = |names: &[&str]| names.iter().any(|n| n.eq_ignore_ascii_case(command));
    if is(&["write", "mkdir", "rm", "mv", "sync", "screenshot"]) { Some("files") }
    else if is(&["ping", "nslookup", "fetch", "https", "net", "netrevoke", "ip", "tls"]) { Some("network") }
    else if is(&["logger"]) { Some("log") }
    else if is(&["kill", "budget", "stop", "reboot"]) { Some("lifecycle") }
    else { None }
}

/// A line typed at the prompt that is a statement of msh rather than a command.
pub fn is_statement(line: &[u8]) -> bool {
    let Ok(text) = core::str::from_utf8(line) else { return false };
    let text = text.trim_start();
    let word = text.split(|c: char| !(c.is_alphanumeric() || c == '_')).next().unwrap_or("");
    if ["let", "if", "while", "for", "fn", "try"].contains(&word) && text[word.len()..].starts_with([' ', '{']) { return true; }
    let rest = &text[word.len()..];
    !word.is_empty() && (rest.starts_with('(') || { let r = rest.trim_start(); r.starts_with('=') && !r.starts_with("==") })
}

struct ShellHost<'a> { shell: &'a mut Shell, requires: Option<Vec<String>>, stop: bool }

impl ShellHost<'_> {
    fn allowed(&self, word: &str) -> bool { self.requires.as_ref().is_none_or(|r| r.iter().any(|w| w == word)) }

    // Runs a command line; its output, captured (returned) or shown; err when it printed an ERROR line.
    fn run(&mut self, line: &str, keep: bool) -> Result<String, String> {
        let word = line.split_whitespace().next().unwrap_or("");
        if let Some(need) = needs(word) {
            if !self.allowed(need) { return Err(alloc::format!("{} needs `requires: {}` in the script", word, need)); }
        }
        let shell = &mut *self.shell;
        let outer = shell.term.capture.replace(Vec::new());
        shell.command(line.as_bytes());
        // A program it started runs to its end (a console program) or until it leaves the foreground.
        let program = shell.console.or(shell.focused);
        loop {
            if let Some(pid) = shell.console {
                shell.pump_console(pid);
                if shell.console.is_none() { continue; }
                if interrupted(shell) { let _ = control::kill(pid); self.stop = true; }
                mind::time::sleep(10);
                continue;
            }
            if let Some(pid) = shell.focused {
                shell.mirror(pid);
                shell.mirror_fronts();
                match control::notice() {
                    Some(control::Notice::Exited(p) | control::Notice::Background(p)) if p == pid => { shell.mirror(pid); shell.drop_fronts(shell.active); shell.focused = None; }
                    Some(control::Notice::Front(p)) => shell.add_front(p),
                    _ => { if !mind::process::alive(pid) { shell.focused = None; } }
                }
                if shell.focused.is_some() { mind::time::sleep(10); continue; }
            }
            break;
        }
        let bytes = core::mem::replace(&mut shell.term.capture, outer).unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if !keep { for &byte in &bytes { shell.term.print_char(byte); } }
        if let Some(reason) = text.lines().find_map(|l| l.strip_prefix("ERROR: ")) { return Err(reason.to_string()); }
        // How the program ended (issue 166): a code other than 0, a kill or a fault is a failure.
        match program.filter(|&pid| !mind::process::alive(pid)).and_then(control::exit_status) {
            Some(Exit::Code(0)) | None => Ok(text),
            Some(Exit::Code(code)) => Err(alloc::format!("{} exited with {}", word, code)),
            Some(Exit::Killed) => Err(alloc::format!("{} was stopped", word)),
            Some(Exit::Fault(vector)) => Err(alloc::format!("{} ended with a fault (vector {})", word, vector)),
        }
    }
}

// Esc or Ctrl+C on either keyboard, Ctrl+Z on the serial line (the kernel keeps PS/2 Ctrl+Z for the focus): a
// script stops. What else was typed meanwhile is dropped.
fn interrupted(shell: &mut Shell) -> bool {
    let mut stop = false;
    while let Some(byte) = shell.term.serial.as_ref().and_then(mind::dev::Uart::read) { if matches!(byte, 0x1A | 0x03 | 0x1B) { stop = true; } }
    while let Some(key) = mind::input::read_key() { if key.is_escape() || key.is_ctrl('c') || key.is_ctrl('z') { stop = true; } }
    stop
}

impl Host for ShellHost<'_> {
    fn command(&mut self, words: &[String]) -> Value {
        match self.run(&words.join(" "), false) { Ok(_) => Value::ok(Value::Nil), Err(reason) => Value::Err(alloc::boxed::Box::new(Value::Str(reason))) }
    }
    fn call(&mut self, name: &str, args: &[Value]) -> Option<Value> {
        let text = |i: usize| match args.get(i) { Some(Value::Str(s)) => Some(s.clone()), _ => None };
        Some(match name {
            "capture" => match text(0) {
                Some(line) => match self.run(&line, true) { Ok(out) => Value::ok(Value::Str(out)), Err(reason) => Value::err(&reason) },
                None => Value::err("capture needs a command line"),
            },
            "ps" => {
                let (list, count) = crate::tasks();
                Value::ok(Value::List(list[..count].iter().map(|t| Value::record(&[
                    ("pid", Value::Int(t.pid as i64)), ("name", Value::str(crate::label(&t.name))), ("state", Value::str(crate::label(&t.state))),
                    ("cpu", Value::Int(t.cpu as i64)), ("service", Value::Bool(t.service != 0)), ("focus", Value::Bool(t.focus != 0)),
                    ("console", self.shell.owner(t.pid).map_or(Value::Nil, |c| Value::Int(c as i64 + 1))),
                ])).collect()))
            }
            "services" => {
                let (list, count) = crate::tasks();
                Value::ok(Value::List(list[..count].iter().filter(|t| t.service != 0).map(|t| Value::str(crate::label(&t.name))).collect()))
            }
            "files" => {
                let dir = text(0).unwrap_or_default();
                let mut entries = Vec::new();
                match mind::fs::list(&dir, |e| entries.push(Value::record(&[("name", Value::str(e.name_str())), ("size", Value::Int(e.size as i64)), ("dir", Value::Bool(e.is_dir))]))) {
                    Ok(_) => Value::ok(Value::List(entries)),
                    Err(e) => Value::err(crate::files::text(e)),
                }
            }
            // glob("ram:*.txt"): the paths in a directory whose names match the mask (`*`, `?`, any case).
            "glob" => {
                let Some(pattern) = text(0) else { return Some(Value::err("glob needs a mask")) };
                let (dir, mask) = match pattern.rfind(['/', ':']) { Some(i) => (&pattern[..=i], &pattern[i + 1..]), None => ("", pattern.as_str()) };
                let mut paths = Vec::new();
                match mind::fs::list(dir.trim_end_matches('/'), |e| if mind::mask::glob(mask, e.name_str()) { paths.push(Value::Str(alloc::format!("{}{}", dir, e.name_str()))); }) {
                    Ok(_) => Value::ok(Value::List(paths)),
                    Err(e) => Value::err(crate::files::text(e)),
                }
            }
            "log" => {
                if !self.allowed("log") { return Some(Value::err("log needs `requires: log` in the script")); }
                let Some(line) = text(0) else { return Some(Value::err("log needs a text")) };
                match mind::log::write(mind::log::INFO, &line) { Ok(()) => Value::ok(Value::Nil), Err(_) => Value::err("no system log") }
            }
            "sleep" => {
                let Some(Value::Int(ms)) = args.first() else { return Some(Value::err("sleep needs milliseconds")) };
                let until = mind::time::uptime_ms() + (*ms).clamp(0, 3_600_000) as usize;
                while mind::time::uptime_ms() < until && !self.stop {
                    if interrupted(self.shell) { self.stop = true; }
                    mind::time::sleep((until - mind::time::uptime_ms()).min(20));
                }
                Value::ok(Value::Nil)
            }
            "now" => Value::Int(mind::time::uptime_ms() as i64),
            _ => return None,
        })
    }
    fn print(&mut self, text: &str) { let _ = writeln!(self.shell.term, "{}", text); }
    fn interrupted(&mut self) -> bool { self.stop || { self.stop = interrupted(self.shell); self.stop } }
}

// Runs `script` in `interpreter` with the session's authority (None) or what the script declared.
fn execute(shell: &mut Shell, interpreter: &mut Interpreter, script: &Script, requires: Option<Vec<String>>) {
    let outer = shell.script.replace(requires.clone().unwrap_or_else(|| WORDS.iter().map(|w| String::from(*w)).collect()));
    let result = {
        let mut host = ShellHost { shell, requires, stop: false };
        interpreter.run(script, &mut host)
    };
    shell.script = outer;
    if let Err(failure) = result {
        if shell.term.position().col != 0 { shell.term.print_char(b'\n'); }
        let _ = writeln!(shell.term, "SCRIPT {}: {} (LINE {})", if failure.stopped { "STOPPED" } else { "FAILED" }, failure.reason, failure.at.line);
    }
}

/// A statement typed at the prompt: run in the console's interpreter, whose variables stay.
pub fn statement(shell: &mut Shell, line: &[u8]) {
    let Ok(text) = core::str::from_utf8(line) else { return shell.report("NOT UTF-8") };
    match parse(text) {
        Ok(script) => {
            let mut interpreter = core::mem::take(&mut shell.msh);
            execute(shell, &mut interpreter, &script, None);
            shell.msh = interpreter;
        }
        Err(error) => { let _ = writeln!(shell.term, "MSH: {}", error); }
    }
}

// The script's text (at most 64 KiB).
fn read(path: &str) -> Result<String, &'static str> {
    let mut file = mind::fs::File::open(path).map_err(crate::files::text)?;
    if file.size() > MAX_SCRIPT { return Err("LONGER THAN 64 KIB"); }
    let mut bytes = alloc::vec![0u8; file.size()];
    let mut at = 0;
    while at < bytes.len() {
        match file.read(&mut bytes[at..]) { Ok(0) => break, Ok(n) => at += n, Err(e) => return Err(crate::files::text(e)) }
    }
    bytes.truncate(at);
    String::from_utf8(bytes).map_err(|_| "NOT UTF-8")
}

// Asks a yes/no question on the screen and the serial line; the answer is a key on either keyboard.
pub(super) fn ask(shell: &mut Shell, question: &str) -> bool {
    let _ = write!(shell.term, "{} (Y/N) ", question);
    shell.term.render(None);
    let until = mind::time::uptime_ms() + 60_000;
    while mind::time::uptime_ms() < until {
        let byte = shell.term.serial.as_ref().and_then(mind::dev::Uart::read).map(|b| b as char)
            .or_else(|| mind::input::read_key().and_then(|k| if k.is_escape() { Some('n') } else { k.char() }));
        match byte.map(|c| c.to_ascii_lowercase()) {
            Some('y') => { let _ = writeln!(shell.term, "Y"); return true; }
            Some('n') => { let _ = writeln!(shell.term, "N"); return false; }
            _ => { mind::time::sleep(20); }
        }
    }
    let _ = writeln!(shell.term, "(NO ANSWER)");
    false
}

/// `msh file [args]`, `msh -c "code"`, `msh --check file`; `name.msh [args]` is `msh name.msh [args]`.
pub fn command(shell: &mut Shell, args: &[u8]) {
    let Ok(args) = core::str::from_utf8(args) else { return shell.report("NOT UTF-8") };
    let args = args.trim();
    if let Some(code) = args.strip_prefix("-c") {
        let code = code.trim();
        let code = code.strip_prefix('"').and_then(|c| c.strip_suffix('"')).unwrap_or(code);
        return statement(shell, code.as_bytes());
    }
    let (check, rest) = match args.strip_prefix("--check") { Some(rest) => (true, rest.trim()), None => (false, args) };
    let mut words = rest.split_whitespace();
    let Some(path) = words.next() else { return shell.report("USAGE: MSH <FILE> [ARGS], MSH -C \"CODE\", MSH --CHECK <FILE>") };
    let source = match read(path) { Ok(s) => s, Err(e) => return shell.report(&alloc::format!("MSH: {}: {}", path, e)) };
    let script = match parse(&source) { Ok(s) => s, Err(e) => { let _ = writeln!(shell.term, "MSH: {}: {}", path, e); return; } };
    if let Some(word) = script.requires.iter().find(|w| !WORDS.contains(&w.as_str())) {
        let _ = writeln!(shell.term, "MSH: {}: requires: unknown word {} (the words of mind::request!: {})", path, word, WORDS.join(" "));
        return;
    }
    if check {
        let unknown = Interpreter::unknown_calls(&script, &HOST_FUNCTIONS);
        for (name, at) in &unknown { let _ = writeln!(shell.term, "MSH: {}: line {}, column {}: no function {}", path, at.line, at.column, name); }
        if unknown.is_empty() { let _ = writeln!(shell.term, "MSH: {}: OK, REQUIRES: {}", path, if script.requires.is_empty() { String::from("NOTHING") } else { script.requires.join(" ") }); }
        return;
    }
    // A script from outside the boot disk (ram:, a USB disk) asks before it uses what it declares.
    let (volume, _) = mind::fs::split(path);
    let boot_disk = volume.is_empty() || volume.eq_ignore_ascii_case("a");
    if !script.requires.is_empty() && !boot_disk && !ask(shell, &alloc::format!("SCRIPT {} REQUIRES {}. ALLOW?", path, script.requires.join(" "))) {
        let _ = writeln!(shell.term, "MSH: {}: NOT RUN", path);
        return;
    }
    let mut interpreter = Interpreter::default();
    interpreter.set("args", Value::List(words.map(Value::str).collect()));
    let requires = script.requires.clone();
    execute(shell, &mut interpreter, &script, Some(requires));
}

/// Whether a command name is a script file to run.
pub fn is_script(name: &[u8]) -> bool { name.len() > 4 && name[name.len() - 4..].eq_ignore_ascii_case(b".msh") }
