//! Host tests of `msh` (libmind/src/script, issue 094): parse errors with line and column; expressions, strings and
//! records; results with `?`, `or` and `try`; command lines against a host; functions, loops and the budgets that
//! stop a script.
#![allow(dead_code, unused_imports)]
extern crate alloc;
#[path = "../libmind/src/mask.rs"]
mod mask;
#[path = "../libmind/src/pattern.rs"]
mod pattern;
#[path = "../libmind/src/script/mod.rs"]
mod script;

use script::{parse, At, Host, Interpreter, Limits, Value};

// A host that keeps what it was asked: the command `fails` fails, `cat` of a name with "missing" fails.
#[derive(Default)]
struct Mock { commands: Vec<Vec<String>>, printed: Vec<String>, polls: usize, stop_after: Option<usize> }

impl Host for Mock {
    fn command(&mut self, words: &[String]) -> Value {
        self.commands.push(words.to_vec());
        match words.first().map(String::as_str) {
            Some("fails") => Value::err("it failed"),
            Some("cat") if words.get(1).is_some_and(|w| w.contains("missing")) => Value::err("no such file"),
            _ => Value::ok(Value::Nil),
        }
    }
    fn call(&mut self, name: &str, args: &[Value]) -> Option<Value> {
        match name {
            "capture" => Some(match args.first() { Some(Value::Str(c)) if c == "ps" => Value::ok(Value::str("1 init RUNNING\n8 vfs_server IPC_WAIT\n")), _ => Value::err("unknown command") }),
            "files" => Some(Value::ok(Value::List(vec![Value::record(&[("name", Value::str("a.txt")), ("size", Value::Int(3)), ("dir", Value::Bool(false))]),
                                                       Value::record(&[("name", Value::str("sub")), ("size", Value::Int(0)), ("dir", Value::Bool(true))]),
                                                       Value::record(&[("name", Value::str("b.txt")), ("size", Value::Int(4)), ("dir", Value::Bool(false))])]))),
            _ => None,
        }
    }
    fn print(&mut self, text: &str) { self.printed.push(text.to_string()); }
    fn interrupted(&mut self) -> bool { self.polls += 1; self.stop_after.is_some_and(|n| self.polls >= n) }
}

fn run(source: &str) -> (Result<(), script::Failure>, Mock, Interpreter) {
    let script = parse(source).unwrap_or_else(|e| panic!("{}: {}", source, e));
    let mut host = Mock::default();
    let mut interpreter = Interpreter::default();
    let result = interpreter.run(&script, &mut host);
    (result, host, interpreter)
}

fn value(source: &str) -> Value {
    let (result, _, interpreter) = run(&format!("let result = {}", source));
    result.unwrap_or_else(|e| panic!("{}: {}", source, e));
    interpreter.get("result").cloned().unwrap()
}

fn error(source: &str) -> (String, u32, u32) {
    let e = parse(source).expect_err(source);
    (e.message, e.at.line, e.at.column)
}

#[test]
fn parse_errors_say_where() {
    assert_eq!(error("let = 3"), ("expected a name".into(), 1, 5));
    assert_eq!(error("let x = 1\nlet y = \"open"), ("unclosed string".into(), 2, 9));
    assert_eq!(error("if true {\n  print(1)\n"), ("expected '}'".into(), 3, 1));
    assert_eq!(error("}"), ("unexpected '}'".into(), 1, 1));
    assert_eq!(error("let x = (1 + 2"), ("expected ')'".into(), 1, 15));
    assert_eq!(error("let x = 1 2"), ("expected the end of the line".into(), 1, 11));
    assert_eq!(error("fn f() {}\nfn f() {}").0, "function f defined twice");
    assert_eq!(error("let s = \"a\\q\"").0, "unknown escape (\\n \\t \\\" \\\\ \\{ \\})");
    // What a script declares it needs, on its first line after `#!msh` and comments.
    let script = parse("#!msh\n# sums the files\nrequires: files network  # and nothing else\nlet x = 1").unwrap();
    assert_eq!(script.requires, ["files", "network"]);
    assert_eq!(error("let x = 1\nrequires: files"), ("requires: must come before the statements".into(), 2, 1));
}

#[test]
fn expressions_strings_lists_records() {
    assert_eq!(value("1 + 2 * 3 - 8 / 4 % 3"), Value::Int(5));
    assert_eq!(value("-(2 + 3) * 2"), Value::Int(-10));
    assert_eq!(value("1 < 2 && !(3 >= 4) || false"), Value::Bool(true));
    assert_eq!(value("\"ab\" < \"b\""), Value::Bool(true));
    assert_eq!(value("[1, 2] + [3]"), Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]));
    assert_eq!(value("{name: \"x\", size: 3}.size"), Value::Int(3));
    assert_eq!(value("[10, 20, 30][-1]"), Value::Int(30));
    assert_eq!(value("{a: 1}[\"a\"]"), Value::Int(1));
    // Interpolation: any expression in braces; \{ is a brace.
    let (result, _, interpreter) = run("let f = {name: \"a.txt\", size: 3}\nlet s = \"{f.name}: {f.size * 2} bytes \\{x\\} {[1, \"two\"]}\"");
    result.unwrap();
    assert_eq!(interpreter.get("s"), Some(&Value::str("a.txt: 6 bytes {x} [1, \"two\"]")));
    assert_eq!(value("\"n=\" + 4 + true"), Value::str("n=4true"));
    // Built-ins.
    assert_eq!(value("lines(\"a\\nb\\n\")"), Value::List(vec![Value::str("a"), Value::str("b")]));
    assert_eq!(value("join(words(\"  x  y \"), \"-\")"), Value::str("x-y"));
    assert_eq!(value("split(\"a,b\", \",\")"), Value::List(vec![Value::str("a"), Value::str("b")]));
    assert_eq!(value("[contains(\"hello\", \"ell\"), contains([1, 2], 2), contains({a: 1}, \"b\")]"), Value::List(vec![Value::Bool(true), Value::Bool(true), Value::Bool(false)]));
    assert_eq!(value("[match(\"vfs_server IPC_WAIT\", \"^vfs.*WAIT$\"), match(\"x\", \"^y\")]"), Value::List(vec![Value::Bool(true), Value::Bool(false)]));
    assert_eq!(value("len(range(0, 5)) + len(\"жук\") + len(keys({a: 1, b: 2}))"), Value::Int(10));
    assert_eq!(value("[int(\" 42 \"), int(\"x\")]"), Value::List(vec![Value::Int(42), Value::err("not an integer")]));
    assert_eq!(value("upper(replace(trim(\" a-b \"), \"-\", \"+\"))"), Value::str("A+B"));
    assert_eq!(value("type(push([], nil))"), Value::str("list"));
}

#[test]
fn results_propagate_and_are_handled() {
    // ? takes ok's value; err fails the script where it is.
    let (result, ..) = run("let out = capture(\"ps\")?\nlet n = len(lines(out))\nlet bad = capture(\"nope\")?\nprint(\"not reached\")");
    let failure = result.unwrap_err();
    assert_eq!((failure.reason.as_str(), failure.at, failure.stopped), ("unknown command", At { line: 3, column: 26 }, false));
    // `or`: a value, or a block that sees `error`.
    assert_eq!(value("capture(\"nope\") or \"none\""), Value::str("none"));
    assert_eq!(value("capture(\"ps\") or \"none\""), Value::str("1 init RUNNING\n8 vfs_server IPC_WAIT\n"));
    let (result, host, interpreter) = run("let r = capture(\"nope\") or {\n  print(\"handled: {error}\")\n  0\n}");
    result.unwrap();
    assert_eq!((interpreter.get("r"), host.printed.as_slice()), (Some(&Value::Int(0)), &["handled: unknown command".to_string()][..]));
    // A result kept in a variable fails nothing until it is used as a value.
    let (result, ..) = run("let r = capture(\"nope\")\nif is_err(r) { print(\"err\") }\nlet n = r + 1");
    assert!(result.unwrap_err().reason.contains("a result: take its value with ? or or"));
    // try / catch, fail, and err(r) as a statement.
    let (result, host, _) = run("try {\n  fail(\"boom\")\n  print(\"no\")\n} catch e {\n  print(\"caught {e}\")\n}\nerr(\"last\")\nprint(\"no\")");
    assert_eq!(result.unwrap_err().reason, "last");
    assert_eq!(host.printed, ["caught boom"]);
    assert_eq!(value("unwrap(ok(5))"), Value::Int(5));
}

#[test]
fn commands_run_through_the_host() {
    let (result, host, _) = run("let dir = \"ram:\"\nls {dir}\nwrite ram:summary.txt \"total {1 + 2} bytes\"\ncat ram:missing.txt or { print(\"missing: {error}\") }\ncat ram:a.txt ?");
    result.unwrap();
    assert_eq!(host.commands, [vec!["ls", "ram:"], vec!["write", "ram:summary.txt", "total 3 bytes"], vec!["cat", "ram:missing.txt"], vec!["cat", "ram:a.txt"]]);
    assert_eq!(host.printed, ["missing: no such file"]);
    // A command that fails stops the script: nothing goes on silently.
    let (result, host, _) = run("fails\nls");
    assert_eq!(result.unwrap_err().reason, "it failed");
    assert_eq!(host.commands.len(), 1);
    // Commands in blocks on one line; a name followed by ( is a call, by = an assignment.
    let (result, host, interpreter) = run("let n = 0\nfor f in files(\"ram:\")? { if f.dir { continue }; n = n + f.size; echo {f.name} }");
    result.unwrap();
    assert_eq!((interpreter.get("n"), host.commands.len()), (Some(&Value::Int(7)), 2));
}

#[test]
fn functions_loops_and_scopes() {
    let (result, host, _) = run("fn fact(n) {\n  if n <= 1 { return 1 }\n  return n * fact(n - 1)\n}\nprint(fact(10))\nlet i = 0\nwhile true {\n  i = i + 1\n  if i == 3 { continue }\n  if i > 5 { break }\n  print(i)\n}");
    result.unwrap();
    assert_eq!(host.printed, ["3628800", "1", "2", "4", "5"]);
    // A function sees its parameters and the outermost variables, not its caller's.
    let (result, ..) = run("let top = 1\nfn f() { return top + hidden }\nfn g() { let hidden = 2\n return f() }\ng()");
    assert_eq!(result.unwrap_err().reason, "hidden is not defined");
    // Assignment needs a let first; a let in a block ends with it.
    assert_eq!(run("x = 1").0.unwrap_err().reason, "x is not defined (let x = …)");
    assert_eq!(run("if true { let y = 1 }\nprint(y)").0.unwrap_err().reason, "y is not defined");
    assert_eq!(run("fn f(a, b) { return a }\nf(1)").0.unwrap_err().reason, "f takes 2 arguments, not 1");
    assert_eq!(run("if 1 { }").0.unwrap_err().reason, "a condition must be true or false, not integer");
    // The prompt's statements share variables and functions between runs.
    let mut interpreter = Interpreter::default();
    let mut host = Mock::default();
    interpreter.run(&parse("let total = 5\nfn twice(x) { return x * 2 }").unwrap(), &mut host).unwrap();
    interpreter.run(&parse("print(twice(total))").unwrap(), &mut host).unwrap();
    assert_eq!(host.printed, ["10"]);
}

#[test]
fn budgets_stop_a_script() {
    // An endless loop spends the step budget; the host can stop it sooner (Ctrl+Z).
    let script = parse("let i = 0\nwhile true { i = i + 1 }").unwrap();
    let mut interpreter = Interpreter::new(Limits { steps: 10_000, ..Limits::default() });
    let failure = interpreter.run(&script, &mut Mock::default()).unwrap_err();
    assert!(failure.stopped && failure.reason.contains("step budget"), "{:?}", failure);
    let mut host = Mock { stop_after: Some(3), ..Mock::default() };
    let failure = Interpreter::default().run(&script, &mut host).unwrap_err();
    assert_eq!((failure.reason.as_str(), failure.stopped), ("stopped", true));
    // try does not catch a stop.
    let script = parse("try { while true { } } catch e { print(\"no\") }").unwrap();
    assert!(Interpreter::new(Limits { steps: 1000, ..Limits::default() }).run(&script, &mut Mock::default()).unwrap_err().stopped);
    // Sizes and depth.
    assert!(run("let s = \"x\"\nwhile true { s = s + s }").0.unwrap_err().reason.starts_with("a string longer than"));
    assert!(run("fn f(n) { return f(n + 1) }\nf(0)").0.unwrap_err().reason.starts_with("calls nested deeper than"));
    assert_eq!(run("let x = 9223372036854775807 + 1").0.unwrap_err().reason, "integer overflow");
    assert_eq!(run("let x = 1 / 0").0.unwrap_err().reason, "division by zero");
}

#[test]
fn check_names_unknown_calls() {
    let script = parse("fn mine() { return 1 }\nprint(mine())\nlet p = ps()\nnope(1)\nif true { also_nope() }").unwrap();
    let unknown: Vec<String> = Interpreter::unknown_calls(&script, &["ps"]).into_iter().map(|(n, _)| n).collect();
    assert_eq!(unknown, ["nope", "also_nope"]);
}
