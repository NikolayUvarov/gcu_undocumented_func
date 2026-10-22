//! Host tests of tests/hardware/wifijoin.msh (550-ASR-0007), the MacBook Pro's join at start, run by the shell's own
//! interpreter against a host that answers as the Mac does: the join tried again when a scan at start misses the network.
#![allow(dead_code, unused_imports)]
extern crate alloc;
#[path = "../libmind/src/mask.rs"]
mod mask;
#[path = "../libmind/src/pattern.rs"]
mod pattern;
#[path = "../libmind/src/script/mod.rs"]
mod script;
use script::{parse, Host, Interpreter, Value};

// The Mac as the script sees it: `wifi join` fails `fail_joins` times, then joins; time moves only when asked.
struct Mac { joins: usize, fail_joins: usize, clock: i64, printed: Vec<String>, captured: Vec<String> }
impl Host for Mac {
    fn command(&mut self, _w: &[String]) -> Value { Value::ok(Value::Nil) }
    fn call(&mut self, name: &str, args: &[Value]) -> Option<Value> {
        let arg = match args.first() { Some(Value::Str(s)) => s.to_string(), _ => String::new() };
        match name {
            "now" => { self.clock += 100; Some(Value::Int(self.clock)) }
            "sleep" => { if let Some(Value::Int(ms)) = args.first() { self.clock += *ms; } Some(Value::Nil) }
            "log" => Some(Value::ok(Value::Nil)),
            "capture" => {
                self.captured.push(arg.clone());
                Some(if arg == "cat data/wifijoin.txt" { Value::ok(Value::str("CoolNet2\n")) }
                else if arg == "wifi status" { Value::ok(Value::str("[WIFI] STATUS OFF\n")) }
                else if arg.starts_with("wifi join") { self.joins += 1; if self.joins <= self.fail_joins { Value::err("wifi exited with 1") } else { Value::ok(Value::str("[WIFI] JOINED CoolNet2\n")) } }
                else if arg == "ip" { Value::ok(Value::str("CARD 2 MAC B8:F6 IP 192.168.2.236/24 GATEWAY 192.168.2.1 SENT=2\n")) }
                else if arg.starts_with("ping ") { Value::ok(Value::str("PING: 3 SENT, 3 RECEIVED\n")) }
                else { Value::err("unknown") })
            }
            _ => None,
        }
    }
    fn print(&mut self, t: &str) { self.printed.push(t.to_string()); }
}

fn run(fail_joins: usize) -> Mac {
    let source = include_str!("hardware/wifijoin.msh").to_string();
    let script = parse(&source).unwrap_or_else(|e| panic!("parse: {} at {}:{}", e.message, e.at.line, e.at.column));
    let mut mac = Mac { joins: 0, fail_joins, clock: 0, printed: vec![], captured: vec![] };
    Interpreter::default().run(&script, &mut mac).unwrap_or_else(|e| panic!("run: {}", e));
    mac
}

#[test]
fn joins_on_the_third_try() {
    let mac = run(2);
    assert_eq!(mac.joins, 3);
    assert!(mac.printed.iter().any(|l| l.starts_with("WIFI JOIN: TRUE AFTER") && l.ends_with("3 TRIES")), "{:#?}", mac.printed);
    assert!(mac.captured.iter().any(|c| c == "ping 192.168.2.1"), "{:#?}", mac.captured);
    assert!(mac.printed.last().unwrap().contains("Complete. Data collected"));
}

#[test]
fn gives_up_after_five() {
    let mac = run(9);
    assert_eq!(mac.joins, 5);
    assert!(mac.printed.iter().any(|l| l.starts_with("WIFI JOIN: FALSE AFTER") && l.ends_with("5 TRIES")), "{:#?}", mac.printed);
    assert!(!mac.captured.iter().any(|c| c.starts_with("ping")));
}

#[test]
fn joins_at_once() { assert_eq!(run(0).joins, 1); }
