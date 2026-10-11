//! Host tests of the runs on real hardware (176-KRN-0087): diag/quick.msh and diag/full.msh run against a shell that
//! answers like the real one, and must make their directory, write every step's output and a summary, record a
//! failed step and go on, and split a step's output over files where it would not fit one string.
#![allow(dead_code, unused_imports)]
extern crate alloc;
#[path = "../libmind/src/mask.rs"]
mod mask;
#[path = "../libmind/src/pattern.rs"]
mod pattern;
#[path = "../libmind/src/script/mod.rs"]
mod script;

use script::{parse, Host, Interpreter, Value};
use std::collections::BTreeMap;

// The words of mind::request! the shell knows (shell/src/msh.rs).
const WORDS: [&str; 15] = ["console", "sysinfo", "file", "files", "lifecycle", "log", "network", "authority", "window", "window-manager", "display", "gpio", "camera", "parse", "tls"];
// The functions the shell gives scripts (shell/src/msh.rs).
const HOST_FUNCTIONS: [&str; 8] = ["capture", "ps", "services", "files", "glob", "log", "sleep", "now"];

// A shell as scripts see it: files written, directories made, commands run; `svc list` fails as on a machine where it
// exits with 1, and a task's buffered output is 60 800 bytes. With `ring_moved`, init's build line is no longer in
// the system log's ring, only in this boot's log file.
#[derive(Default)]
struct Shell { files: BTreeMap<String, String>, dirs: Vec<String>, run: Vec<String>, printed: Vec<String>, logged: Vec<String>, clock: i64, ring_moved: bool }

impl Shell {
    fn capture(&mut self, line: &str) -> Value {
        self.run.push(line.to_string());
        if let Some(rest) = line.strip_prefix("write ") {
            let (path, body) = rest.split_once(' ').unwrap_or((rest, ""));
            self.files.insert(path.to_string(), body.trim().to_string() + "\n");
            return Value::ok(Value::str(&format!("WROTE {} BYTES TO {}\n", body.trim().len() + 1, path)));
        }
        if let Some(dir) = line.strip_prefix("mkdir ") { self.dirs.push(dir.to_string()); return Value::ok(Value::str("OK\n")); }
        match line {
            "svc list" => Value::err("svc exited with 1"),
            "date" => Value::ok(Value::str("2026-10-11 18:00:00\n")),
            "dmesg -s init" if self.ring_moved => Value::ok(Value::str("[  412.001] init(1) [INIT] STARTED wm PID=40\n")),
            "dmesg -s init" => Value::ok(Value::str("[    0.250] init(1) [INIT] BUILD: BRANCH fast-test, COMMIT 8c15a16abcde\n")),
            "grep BUILD: log:boot0003.log" => Value::ok(Value::str("[    0.250] init(1) [INIT] BUILD: BRANCH main, COMMIT 3b64839abcde\n")),
            _ if line.starts_with("logs ") => Value::ok(Value::str(&"output of the task\n".repeat(3200))),
            _ => Value::ok(Value::str(&format!("what {} printed\n", line))),
        }
    }
}

impl Host for Shell {
    fn command(&mut self, words: &[String]) -> Value { self.capture(&words.join(" ")) }
    fn call(&mut self, name: &str, args: &[Value]) -> Option<Value> {
        let text = |i: usize| match args.get(i) { Some(Value::Str(s)) => s.clone(), _ => String::new() };
        Some(match name {
            "capture" => self.capture(&text(0)),
            "glob" => Value::ok(Value::List(vec![Value::str("log:BOOT0002.LOG"), Value::str("log:boot0003.log")])),
            // A run of this kind already made today: the next one gets -2.
            "files" => Value::ok(Value::List(vec![Value::record(&[("name", Value::str("QUICK0003")), ("size", Value::Int(0)), ("dir", Value::Bool(true))]),
                                                  Value::record(&[("name", Value::str("full0003")), ("size", Value::Int(0)), ("dir", Value::Bool(true))])])),
            "ps" => Value::ok(Value::List(vec![
                Value::record(&[("pid", Value::Int(1)), ("name", Value::str("init")), ("state", Value::str("IPC_WAIT")), ("cpu", Value::Int(0)), ("service", Value::Bool(true)), ("focus", Value::Bool(false)), ("console", Value::Nil)]),
                Value::record(&[("pid", Value::Int(7)), ("name", Value::str("wm")), ("state", Value::str("SLEEPING")), ("cpu", Value::Int(2)), ("service", Value::Bool(false)), ("focus", Value::Bool(true)), ("console", Value::Int(1))]),
            ])),
            "services" => Value::ok(Value::List(vec![Value::str("init"), Value::str("logd")])),
            "log" => { self.logged.push(text(0)); Value::ok(Value::Nil) }
            "sleep" => Value::ok(Value::Nil),
            "now" => { self.clock += 7; Value::Int(self.clock) }
            _ => return None,
        })
    }
    fn print(&mut self, text: &str) { self.printed.push(text.to_string()); }
    fn interrupted(&mut self) -> bool { false }
}

fn run(path: &str, ring_moved: bool) -> Shell {
    let source = std::fs::read_to_string(path).unwrap();
    let script = parse(&source).unwrap_or_else(|e| panic!("{}: {:?}", path, e));
    assert!(script.requires.iter().all(|w| WORDS.contains(&w.as_str())), "{}: {:?}", path, script.requires);
    assert!(Interpreter::unknown_calls(&script, &HOST_FUNCTIONS).is_empty(), "{}: {:?}", path, Interpreter::unknown_calls(&script, &HOST_FUNCTIONS));
    let mut shell = Shell { ring_moved, ..Shell::default() };
    let result = Interpreter::default().run(&script, &mut shell);
    assert!(result.is_ok(), "{}: {:?}\n{:?}", path, result.err(), shell.printed);
    shell
}

#[test]
fn the_quick_run_writes_each_step_and_a_summary() {
    let shell = run("diag/quick.msh", false);
    assert_eq!(shell.dirs, ["log:quick0003-2"]);
    let summary = &shell.files["log:quick0003-2/summary.txt"];
    assert!(summary.contains("BUILD: BRANCH fast-test, COMMIT 8c15a16abcde") && !shell.run.iter().any(|c| c.starts_with("grep")), "{}", summary);
    assert!(summary.contains("19 steps, 1 failed"), "{}", summary);
    assert!(summary.contains("06 FAILED") && summary.contains("svc list: svc exited with 1"), "{}", summary);
    assert!(summary.contains("17 ok") && summary.contains("bench files"), "{}", summary);
    assert!(summary.contains("000-KRN-0086") && !summary.contains("RUNNING NOW"), "{}", summary);
    // Every step's output in its own file, in order; a failed one says why.
    assert_eq!(shell.files["log:quick0003-2/16-kbench.txt"], "what kbench timer ipc printed\n");
    assert_eq!(shell.files["log:quick0003-2/06-svc.txt"], "FAILED: svc exited with 1\n");
    assert_eq!(shell.files.keys().filter(|k| k.starts_with("log:quick0003-2/")).count(), 20);
    // Each step is in the system log before it runs, so a machine that stops in one names it.
    assert!(shell.logged.iter().any(|l| l == "DIAG QUICK: STEP 16 OF 19: kbench timer ipc"), "{:?}", shell.logged);
    assert!(shell.printed.iter().any(|p| p.starts_with("DIAG QUICK: 19 STEPS, 1 FAILED")), "{:?}", shell.printed);
}

#[test]
fn the_full_run_covers_every_task_and_every_measurement() {
    let shell = run("diag/full.msh", true);
    assert_eq!(shell.dirs, ["log:full0003-2"]);
    let summary = &shell.files["log:full0003-2/summary.txt"];
    // The build from this boot's log file; 34 views before, two tasks, three measurements, six after.
    assert!(summary.contains("BUILD: BRANCH main, COMMIT 3b64839abcde"), "{}", summary);
    assert!(summary.contains("45 steps, 1 failed"), "{}", summary);
    for command in ["check", "kbench", "bench", "dmesg", "fsck", "stat irqs", "list -l"] {
        assert!(shell.run.iter().any(|c| c == command), "{} not run", command);
    }
    // A task's views in one file; its 60 800 bytes of buffered output would not fit the same string, so they go on
    // in -2.
    let wm = &shell.files["log:full0003-2/36-task-7-wm.txt"];
    assert!(wm.starts_with("== stat 7\nwhat stat 7 printed\n") && wm.contains("== caps 7") && wm.contains("== pmap 7"), "{}", &wm[..200]);
    assert!(shell.files["log:full0003-2/36-task-7-wm-2.txt"].starts_with("== logs 7\n"));
    assert!(shell.files.keys().all(|k| shell.files[k].len() <= 65536), "a file longer than one string");
}
