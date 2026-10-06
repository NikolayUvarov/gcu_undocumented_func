//! `msh`, the shell's script language (issue 094): values, results as in Marain (RFC 001 §7.5), commands as the
//! shell runs them. `parse` reads a script; `Interpreter` runs it against a `Host` (the shell: its commands, the
//! system's typed answers, its output), within a step budget and limits on sizes and depth, so no script can hang
//! the shell.
//!
//! - Values: `nil`, booleans, integers (i64), strings, lists, records `{name: "x", size: 3}`, and results `ok(v)`,
//!   `err(r)`.
//! - A command line or a call that ends in `err(r)` stops the script unless handled: `expr?` takes the value of
//!   `ok(v)` and fails on `err`; `expr or { … }` runs the block (`error` is the reason) when `expr` fails;
//!   `try { … } catch e { … }` handles a block. `fail(reason)` fails at once.
pub mod parse;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
pub use parse::{parse, At, ParseError, Script};
use parse::{Expr, Function, Handler, Kind, Op, Part, Stmt};

#[derive(Clone, Debug, PartialEq)]
pub enum Value { Nil, Bool(bool), Int(i64), Str(String), List(Vec<Value>), Record(Vec<(String, Value)>), Ok(Box<Value>), Err(Box<Value>) }

impl Value {
    pub fn str(text: &str) -> Self { Value::Str(String::from(text)) }
    pub fn ok(value: Value) -> Self { Value::Ok(Box::new(value)) }
    pub fn err(reason: &str) -> Self { Value::Err(Box::new(Value::str(reason))) }
    pub fn record(fields: &[(&str, Value)]) -> Self { Value::Record(fields.iter().map(|(k, v)| (String::from(*k), v.clone())).collect()) }
    pub fn type_name(&self) -> &'static str {
        match self { Value::Nil => "nil", Value::Bool(_) => "boolean", Value::Int(_) => "integer", Value::Str(_) => "string", Value::List(_) => "list",
                     Value::Record(_) => "record", Value::Ok(_) => "ok", Value::Err(_) => "err" }
    }
    fn quoted(&self, out: &mut String) {
        match self { Value::Str(s) => { out.push('"'); out.push_str(s); out.push('"'); } other => other.show(out) }
    }
    /// The value as text: a string as it is, the others as they are written.
    pub fn show(&self, out: &mut String) {
        match self {
            Value::Nil => out.push_str("nil"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Int(n) => out.push_str(&n.to_string()),
            Value::Str(s) => out.push_str(s),
            Value::List(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() { if i > 0 { out.push_str(", "); } item.quoted(out); }
                out.push(']');
            }
            Value::Record(fields) => {
                out.push('{');
                for (i, (name, value)) in fields.iter().enumerate() { if i > 0 { out.push_str(", "); } out.push_str(name); out.push_str(": "); value.quoted(out); }
                out.push('}');
            }
            Value::Ok(v) => { out.push_str("ok("); v.quoted(out); out.push(')'); }
            Value::Err(v) => { out.push_str("err("); v.quoted(out); out.push(')'); }
        }
    }
    pub fn text(&self) -> String { let mut out = String::new(); self.show(&mut out); out }
    /// A record's field.
    pub fn field(&self, name: &str) -> Option<&Value> { if let Value::Record(fields) = self { fields.iter().find(|(n, _)| n == name).map(|(_, v)| v) } else { None } }
}

/// What a script runs against: the shell's commands and built-ins, and where it prints.
pub trait Host {
    /// Runs a command line (its words, interpolated): `ok(nil)` or `err(reason)`.
    fn command(&mut self, words: &[String]) -> Value;
    /// A built-in the host provides (`capture`, `ps`, `files`, …); None: no such function.
    fn call(&mut self, name: &str, args: &[Value]) -> Option<Value>;
    fn print(&mut self, text: &str);
    /// Asked every few hundred steps: true stops the script (Ctrl+Z, Esc).
    fn interrupted(&mut self) -> bool { false }
}

/// Why a script stopped before its end.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure { pub reason: String, pub at: At, pub stopped: bool }

impl core::fmt::Display for Failure {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { write!(f, "{} (line {})", self.reason, self.at.line) }
}

/// Budgets: statements and expressions evaluated, string and list sizes, call depth.
#[derive(Clone, Copy, Debug)]
pub struct Limits { pub steps: u64, pub text: usize, pub items: usize, pub depth: usize }

impl Default for Limits { fn default() -> Self { Self { steps: 10_000_000, text: 64 * 1024, items: 16 * 1024, depth: 64 } } }

/// The functions every script has (the host adds its own).
pub const BUILTINS: [&str; 26] = ["ok", "err", "fail", "print", "len", "str", "int", "lines", "words", "split", "join", "trim", "contains", "starts_with", "ends_with",
                                  "match", "push", "range", "keys", "is_ok", "is_err", "upper", "lower", "replace", "type", "unwrap"];

enum Unwind { Fail(Value, At), Stop(String, At), Return(Value), Break, Continue }

type Flow<T> = Result<T, Unwind>;

/// Runs scripts; its variables stay between runs (the prompt's statements share them).
pub struct Interpreter { pub limits: Limits, steps: u64, depth: usize, scopes: Vec<Vec<(String, Value)>>, functions: Vec<Function> }

impl Default for Interpreter { fn default() -> Self { Self::new(Limits::default()) } }

impl Interpreter {
    pub fn new(limits: Limits) -> Self { Self { limits, steps: 0, depth: 0, scopes: alloc::vec![Vec::new()], functions: Vec::new() } }

    /// A variable of the outermost scope (`args`, the prompt's `let`s).
    pub fn set(&mut self, name: &str, value: Value) {
        let scope = &mut self.scopes[0];
        match scope.iter_mut().find(|(n, _)| n == name) { Some(slot) => slot.1 = value, None => scope.push((String::from(name), value)) }
    }
    pub fn get(&self, name: &str) -> Option<&Value> { self.scopes.iter().rev().flat_map(|s| s.iter().rev()).find(|(n, _)| n == name).map(|(_, v)| v) }

    /// Runs `script`; its functions join those of earlier runs (a function defined again replaces the old one).
    pub fn run(&mut self, script: &Script, host: &mut dyn Host) -> Result<(), Failure> {
        for function in &script.functions {
            self.functions.retain(|f| f.name != function.name);
            self.functions.push(function.clone());
        }
        self.steps = 0;
        self.depth = 0;
        self.scopes.truncate(1);
        let result = self.statements(&script.body, host);
        self.scopes.truncate(1);
        match result {
            Ok(_) | Err(Unwind::Return(_)) => Ok(()),
            Err(Unwind::Fail(reason, at)) => Err(Failure { reason: reason.text(), at, stopped: false }),
            Err(Unwind::Stop(reason, at)) => Err(Failure { reason, at, stopped: true }),
            Err(Unwind::Break | Unwind::Continue) => Err(Failure { reason: String::from("break or continue outside a loop"), at: At::default(), stopped: false }),
        }
    }

    fn step(&mut self, at: At, host: &mut dyn Host) -> Flow<()> {
        self.steps += 1;
        if self.steps > self.limits.steps { return Err(Unwind::Stop(String::from("the step budget is spent (an endless loop?)"), at)); }
        if self.steps % 256 == 0 && host.interrupted() { return Err(Unwind::Stop(String::from("stopped"), at)); }
        Ok(())
    }
    fn fail<T>(&self, reason: String, at: At) -> Flow<T> { Err(Unwind::Fail(Value::Str(reason), at)) }

    fn bind(&mut self, name: &str, value: Value) { self.scopes.last_mut().unwrap().push((String::from(name), value)); }
    fn assign(&mut self, name: &str, value: Value, at: At) -> Flow<()> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(slot) = scope.iter_mut().rev().find(|(n, _)| n == name) { slot.1 = value; return Ok(()); }
        }
        self.fail(format!("{} is not defined (let {} = …)", name, name), at)
    }

    // A block in a scope of its own; its value is that of its last statement when that is an expression.
    fn block(&mut self, body: &[Stmt], host: &mut dyn Host) -> Flow<Value> {
        self.scopes.push(Vec::new());
        let result = self.statements(body, host);
        self.scopes.pop();
        result
    }
    fn statements(&mut self, body: &[Stmt], host: &mut dyn Host) -> Flow<Value> {
        let mut last = Value::Nil;
        for stmt in body { last = self.statement(stmt, host)?; }
        Ok(last)
    }
    fn statement(&mut self, stmt: &Stmt, host: &mut dyn Host) -> Flow<Value> {
        self.step(stmt.at, host)?;
        match &stmt.kind {
            Kind::Let(name, value) => { let value = self.value(value, host)?; self.bind(name, value); Ok(Value::Nil) }
            Kind::Assign(name, value) => { let value = self.value(value, host)?; self.assign(name, value, stmt.at)?; Ok(Value::Nil) }
            Kind::If(arms, otherwise) => {
                for (cond, body) in arms { if self.condition(cond, host)? { return self.block(body, host); } }
                match otherwise { Some(body) => self.block(body, host), None => Ok(Value::Nil) }
            }
            Kind::While(cond, body) => {
                while self.condition(cond, host)? {
                    match self.block(body, host) { Err(Unwind::Break) => break, Err(Unwind::Continue) | Ok(_) => {} Err(other) => return Err(other) }
                    self.step(stmt.at, host)?; // an empty body is a step too
                }
                Ok(Value::Nil)
            }
            Kind::For(name, list, body) => {
                let items = match self.value(list, host)? {
                    Value::List(items) => items,
                    other => return self.fail(format!("for needs a list, not {}", other.type_name()), stmt.at),
                };
                for item in items {
                    self.scopes.push(alloc::vec![(name.clone(), item)]);
                    let result = self.statements(body, host);
                    self.scopes.pop();
                    match result { Err(Unwind::Break) => break, Err(Unwind::Continue) | Ok(_) => {} Err(other) => return Err(other) }
                    self.step(stmt.at, host)?;
                }
                Ok(Value::Nil)
            }
            Kind::Break => Err(Unwind::Break),
            Kind::Continue => Err(Unwind::Continue),
            Kind::Return(value) => { let value = match value { Some(v) => self.eval(v, host)?, None => Value::Nil }; Err(Unwind::Return(value)) }
            Kind::Try(body, name, handler) => match self.block(body, host) {
                Err(Unwind::Fail(reason, _)) => {
                    self.scopes.push(alloc::vec![(name.clone(), reason)]);
                    let result = self.statements(handler, host);
                    self.scopes.pop();
                    result
                }
                other => other,
            },
            // A statement that ends in err(r) fails: nothing goes on silently after an error.
            Kind::Expr(expr) => match self.eval(expr, host)? {
                Value::Err(reason) => Err(Unwind::Fail(*reason, stmt.at)),
                value => Ok(value),
            },
        }
    }

    fn condition(&mut self, cond: &Expr, host: &mut dyn Host) -> Flow<bool> {
        match self.value(cond, host)? { Value::Bool(b) => Ok(b), other => self.fail(format!("a condition must be true or false, not {}", other.type_name()), at_of(cond)) }
    }
    // An expression whose value is used: a result must be unwrapped first (? or `or`).
    fn value(&mut self, expr: &Expr, host: &mut dyn Host) -> Flow<Value> {
        let value = self.eval(expr, host)?;
        Ok(value)
    }

    fn sized(&self, value: Value, at: At) -> Flow<Value> {
        match &value {
            Value::Str(s) if s.len() > self.limits.text => self.fail(format!("a string longer than {} bytes", self.limits.text), at),
            Value::List(items) if items.len() > self.limits.items => self.fail(format!("a list longer than {} items", self.limits.items), at),
            _ => Ok(value),
        }
    }

    fn parts(&mut self, parts: &[Part], host: &mut dyn Host) -> Flow<String> {
        let mut text = String::new();
        for part in parts {
            match part {
                Part::Text(t) => text.push_str(t),
                Part::Expr(e) => {
                    let value = self.eval(e, host)?;
                    if let Value::Err(reason) = value { return Err(Unwind::Fail(*reason, at_of(e))); }
                    value.show(&mut text);
                }
            }
            if text.len() > self.limits.text { return self.fail(format!("a string longer than {} bytes", self.limits.text), At::default()); }
        }
        Ok(text)
    }

    fn eval(&mut self, expr: &Expr, host: &mut dyn Host) -> Flow<Value> {
        match expr {
            Expr::Nil => Ok(Value::Nil),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Int(n) => Ok(Value::Int(*n)),
            Expr::Str(parts) => Ok(Value::Str(self.parts(parts, host)?)),
            Expr::Var(name, at) => match self.get(name) {
                Some(value) => Ok(value.clone()),
                None => self.fail(format!("{} is not defined", name), *at),
            },
            Expr::List(items) => {
                let mut list = Vec::new();
                for item in items { list.push(self.eval(item, host)?); }
                Ok(Value::List(list))
            }
            Expr::Record(fields) => {
                let mut record: Vec<(String, Value)> = Vec::new();
                for (name, value) in fields {
                    let value = self.eval(value, host)?;
                    record.retain(|(n, _)| n != name);
                    record.push((name.clone(), value));
                }
                Ok(Value::Record(record))
            }
            Expr::Unary(op, operand) => {
                let value = self.eval(operand, host)?;
                match (op, value) {
                    (Op::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                    (Op::Neg, Value::Int(n)) => Ok(Value::Int(n.wrapping_neg())),
                    (_, other) => self.fail(format!("{} cannot be {}", other.type_name(), if *op == Op::Not { "negated with !" } else { "negated" }), at_of(operand)),
                }
            }
            Expr::Binary(op, left, right, at) => {
                let (a, b) = (self.eval(left, host)?, self.eval(right, host)?);
                let value = self.binary(*op, a, b, *at)?;
                self.sized(value, *at)
            }
            Expr::And(left, right) => {
                if !self.condition(left, host)? { return Ok(Value::Bool(false)); }
                Ok(Value::Bool(self.condition(right, host)?))
            }
            Expr::OrElse(left, right) => {
                if self.condition(left, host)? { return Ok(Value::Bool(true)); }
                Ok(Value::Bool(self.condition(right, host)?))
            }
            Expr::Field(record, name, at) => {
                let value = self.eval(record, host)?;
                match value.field(name) {
                    Some(v) => Ok(v.clone()),
                    None => self.fail(format!("{} has no field {}", value.type_name(), name), *at),
                }
            }
            Expr::Index(list, index, at) => {
                let (list, index) = (self.eval(list, host)?, self.eval(index, host)?);
                match (&list, &index) {
                    (Value::List(items), Value::Int(i)) => {
                        let i = if *i < 0 { items.len() as i64 + i } else { *i };
                        match usize::try_from(i).ok().and_then(|i| items.get(i)) { Some(v) => Ok(v.clone()), None => self.fail(format!("index {} outside a list of {}", i, items.len()), *at) }
                    }
                    (Value::Record(_), Value::Str(name)) => match list.field(name) { Some(v) => Ok(v.clone()), None => self.fail(format!("no field {}", name), *at) },
                    _ => self.fail(format!("{} cannot be indexed by {}", list.type_name(), index.type_name()), *at),
                }
            }
            Expr::Propagate(inner, at) => match self.eval(inner, host)? {
                Value::Ok(v) => Ok(*v),
                Value::Err(reason) => Err(Unwind::Fail(*reason, *at)),
                other => Ok(other),
            },
            Expr::Handle(inner, handler) => {
                let reason = match self.eval(inner, host) {
                    Ok(Value::Ok(v)) => return Ok(*v),
                    Ok(Value::Err(reason)) => *reason,
                    Ok(other) => return Ok(other),
                    Err(Unwind::Fail(reason, _)) => reason,
                    Err(other) => return Err(other),
                };
                match handler {
                    Handler::Expr(e) => {
                        self.scopes.push(alloc::vec![(String::from("error"), reason)]);
                        let value = self.eval(e, host);
                        self.scopes.pop();
                        value
                    }
                    Handler::Block(body) => {
                        self.scopes.push(alloc::vec![(String::from("error"), reason)]);
                        let value = self.statements(body, host);
                        self.scopes.pop();
                        value
                    }
                }
            }
            Expr::Command(words, at) => {
                self.step(*at, host)?;
                let mut texts = Vec::new();
                for word in words { texts.push(self.parts(word, host)?); }
                let value = host.command(&texts);
                if host.interrupted() { return Err(Unwind::Stop(String::from("stopped"), *at)); }
                Ok(value)
            }
            Expr::Call(name, args, at) => {
                self.step(*at, host)?;
                let mut values = Vec::new();
                for arg in args { values.push(self.eval(arg, host)?); }
                self.call(name, values, *at, host)
            }
        }
    }

    fn binary(&self, op: Op, a: Value, b: Value, at: At) -> Flow<Value> {
        use Value::*;
        let result = match (op, a, b) {
            (Op::Eq, a, b) => Bool(a == b),
            (Op::Ne, a, b) => Bool(a != b),
            (Op::Add, Int(x), Int(y)) => Int(x.checked_add(y).ok_or(())?),
            (Op::Sub, Int(x), Int(y)) => Int(x.checked_sub(y).ok_or(())?),
            (Op::Mul, Int(x), Int(y)) => Int(x.checked_mul(y).ok_or(())?),
            (Op::Div | Op::Rem, Int(_), Int(0)) => return self.fail(String::from("division by zero"), at),
            (Op::Div, Int(x), Int(y)) => Int(x.wrapping_div(y)),
            (Op::Rem, Int(x), Int(y)) => Int(x.wrapping_rem(y)),
            (Op::Add, Str(x), y) => { let mut s = x; y.show(&mut s); Str(s) }
            (Op::Add, x @ (Int(_) | Bool(_) | Nil), Str(y)) => { let mut s = x.text(); s.push_str(&y); Str(s) }
            (Op::Add, List(mut x), List(y)) => { x.extend(y); List(x) }
            (Op::Lt | Op::Le | Op::Gt | Op::Ge, x, y) => {
                let ordering = match (&x, &y) {
                    (Int(x), Int(y)) => x.cmp(y),
                    (Str(x), Str(y)) => x.cmp(y),
                    _ => return self.fail(format!("{} and {} cannot be compared", x.type_name(), y.type_name()), at),
                };
                Bool(match op { Op::Lt => ordering.is_lt(), Op::Le => ordering.is_le(), Op::Gt => ordering.is_gt(), _ => ordering.is_ge() })
            }
            (op, x, y) => {
                let hint = if matches!(x, Ok(_) | Err(_)) || matches!(y, Ok(_) | Err(_)) { " (a result: take its value with ? or or)" } else { "" };
                return self.fail(format!("{} {} {} is not defined{}", x.type_name(), symbol(op), y.type_name(), hint), at);
            }
        };
        Result::Ok(result)
    }

    fn call(&mut self, name: &str, args: Vec<Value>, at: At, host: &mut dyn Host) -> Flow<Value> {
        if let Some(index) = self.functions.iter().position(|f| f.name == name) {
            let function = self.functions[index].clone();
            if args.len() != function.params.len() { return self.fail(format!("{} takes {} arguments, not {}", name, function.params.len(), args.len()), at); }
            if self.depth >= self.limits.depth { return self.fail(format!("calls nested deeper than {}", self.limits.depth), at); }
            self.depth += 1;
            // A function sees its parameters and the outermost scope, not its caller's variables.
            let saved = self.scopes.split_off(1);
            self.scopes.push(function.params.iter().cloned().zip(args).collect());
            let result = self.statements(&function.body, host);
            self.scopes.truncate(1);
            self.scopes.extend(saved);
            self.depth -= 1;
            return match result { Ok(_) => Ok(Value::Nil), Err(Unwind::Return(v)) => Ok(v), Err(Unwind::Break | Unwind::Continue) => self.fail(String::from("break or continue outside a loop"), at), Err(other) => Err(other) };
        }
        match builtin(name, &args) {
            Some(Ok(value)) => return self.sized(value, at),
            Some(Err(Thrown::Fail(reason))) => return Err(Unwind::Fail(reason, at)),
            Some(Err(Thrown::Wrong(message))) => return self.fail(format!("{}: {}", name, message), at),
            None => {}
        }
        if name == "print" {
            let text: Vec<String> = args.iter().map(Value::text).collect();
            host.print(&text.join(" "));
            return Ok(Value::Nil);
        }
        match host.call(name, &args) {
            Some(_) if host.interrupted() => Err(Unwind::Stop(String::from("stopped"), at)),
            Some(value) => self.sized(value, at),
            None => self.fail(format!("no function {}", name), at),
        }
    }

    /// Names a script calls that are neither built in nor its own nor in `host_names` (`msh --check`).
    pub fn unknown_calls(script: &Script, host_names: &[&str]) -> Vec<(String, At)> {
        let mut unknown = Vec::new();
        let known = |name: &str| BUILTINS.contains(&name) || host_names.contains(&name) || script.functions.iter().any(|f| f.name == name);
        let mut check = |name: &str, at: At| if !known(name) && !unknown.iter().any(|(n, _): &(String, At)| n == name) { unknown.push((String::from(name), at)); };
        for stmt in script.body.iter().chain(script.functions.iter().flat_map(|f| f.body.iter())) { walk_stmt(stmt, &mut check); }
        unknown
    }
}

fn symbol(op: Op) -> &'static str {
    match op { Op::Add => "+", Op::Sub => "-", Op::Mul => "*", Op::Div => "/", Op::Rem => "%", Op::Eq => "==", Op::Ne => "!=", Op::Lt => "<", Op::Le => "<=", Op::Gt => ">", Op::Ge => ">=", Op::Not => "!", Op::Neg => "-" }
}

// Integer overflow in `binary` (the `?` on `ok_or(())`).
impl From<()> for Unwind { fn from(_: ()) -> Self { Unwind::Fail(Value::str("integer overflow"), At::default()) } }

fn at_of(expr: &Expr) -> At {
    match expr {
        Expr::Var(_, at) | Expr::Binary(_, _, _, at) | Expr::Call(_, _, at) | Expr::Field(_, _, at) | Expr::Index(_, _, at) | Expr::Propagate(_, at) | Expr::Command(_, at) => *at,
        Expr::Unary(_, e) | Expr::And(e, _) | Expr::OrElse(e, _) | Expr::Handle(e, _) => at_of(e),
        _ => At::default(),
    }
}

fn walk_stmt(stmt: &Stmt, check: &mut dyn FnMut(&str, At)) {
    let block = |body: &[Stmt], check: &mut dyn FnMut(&str, At)| for s in body { walk_stmt(s, check); };
    match &stmt.kind {
        Kind::Let(_, e) | Kind::Assign(_, e) | Kind::Expr(e) | Kind::Return(Some(e)) => walk_expr(e, check),
        Kind::If(arms, otherwise) => { for (c, b) in arms { walk_expr(c, check); block(b, check); } if let Some(b) = otherwise { block(b, check); } }
        Kind::While(c, b) | Kind::For(_, c, b) => { walk_expr(c, check); block(b, check); }
        Kind::Try(b, _, h) => { block(b, check); block(h, check); }
        _ => {}
    }
}

fn walk_expr(expr: &Expr, check: &mut dyn FnMut(&str, At)) {
    let parts = |parts: &[Part], check: &mut dyn FnMut(&str, At)| for p in parts { if let Part::Expr(e) = p { walk_expr(e, check); } };
    match expr {
        Expr::Str(p) => parts(p, check),
        Expr::List(items) => for e in items { walk_expr(e, check); },
        Expr::Record(fields) => for (_, e) in fields { walk_expr(e, check); },
        Expr::Unary(_, e) | Expr::Field(e, _, _) | Expr::Propagate(e, _) => walk_expr(e, check),
        Expr::Binary(_, a, b, _) | Expr::And(a, b) | Expr::OrElse(a, b) | Expr::Index(a, b, _) => { walk_expr(a, check); walk_expr(b, check); }
        Expr::Call(name, args, at) => { check(name, *at); for e in args { walk_expr(e, check); } }
        Expr::Handle(e, handler) => {
            walk_expr(e, check);
            match handler { Handler::Expr(h) => walk_expr(h, check), Handler::Block(body) => for s in body { walk_stmt(s, check); } }
        }
        Expr::Command(words, _) => for w in words { parts(w, check); },
        _ => {}
    }
}

enum Thrown { Fail(Value), Wrong(String) }

fn wrong<T>(message: &str) -> Result<T, Thrown> { Err(Thrown::Wrong(String::from(message))) }

// The built-in functions that need nothing from the host (None: not one of them; `print` is the interpreter's).
fn builtin(name: &str, args: &[Value]) -> Option<Result<Value, Thrown>> {
    use Value::*;
    let text = |i: usize| match args.get(i) { Some(Str(s)) => Result::Ok(s.as_str()), _ => wrong("expected a string") };
    let arity = |n: usize| if args.len() == n { Result::Ok(()) } else { Result::Err(Thrown::Wrong(format!("takes {} argument{}", n, if n == 1 { "" } else { "s" }))) };
    let result = (|| -> Result<Value, Thrown> {
        Result::Ok(match name {
            "ok" => { arity(1)?; Value::ok(args[0].clone()) }
            "err" => { arity(1)?; Err(Box::new(args[0].clone())) }
            "fail" => { arity(1)?; return Result::Err(Thrown::Fail(args[0].clone())); }
            "unwrap" => { arity(1)?; match &args[0] { Value::Ok(v) => (**v).clone(), Value::Err(r) => return Result::Err(Thrown::Fail((**r).clone())), other => other.clone() } }
            "len" => { arity(1)?; Int(match &args[0] { Str(s) => s.chars().count(), List(l) => l.len(), Record(r) => r.len(), _ => return wrong("expected a string, list or record") } as i64) }
            "str" => { arity(1)?; Str(args[0].text()) }
            "type" => { arity(1)?; Value::str(args[0].type_name()) }
            "int" => { arity(1)?; match &args[0] { Int(n) => Int(*n), Str(s) => match s.trim().parse() { Result::Ok(n) => Int(n), Result::Err(_) => Value::err("not an integer") }, _ => return wrong("expected a string") } }
            "lines" => { arity(1)?; List(text(0)?.lines().map(Value::str).collect()) }
            "words" => { arity(1)?; List(text(0)?.split_whitespace().map(Value::str).collect()) }
            "split" => { arity(2)?; let sep = text(1)?; if sep.is_empty() { return wrong("an empty separator"); } List(text(0)?.split(sep).map(Value::str).collect()) }
            "join" => {
                arity(2)?;
                let List(items) = &args[0] else { return wrong("expected a list") };
                Str(items.iter().map(Value::text).collect::<Vec<_>>().join(text(1)?))
            }
            "trim" => { arity(1)?; Value::str(text(0)?.trim()) }
            "upper" => { arity(1)?; Str(text(0)?.to_uppercase()) }
            "lower" => { arity(1)?; Str(text(0)?.to_lowercase()) }
            "replace" => { arity(3)?; Str(text(0)?.replace(text(1)?, text(2)?)) }
            "contains" => {
                arity(2)?;
                Bool(match (&args[0], &args[1]) { (Str(s), Str(t)) => s.contains(t.as_str()), (List(l), v) => l.contains(v), (Record(_), Str(n)) => args[0].field(n).is_some(), _ => return wrong("expected a string, list or record") })
            }
            "starts_with" => { arity(2)?; Bool(text(0)?.starts_with(text(1)?)) }
            "ends_with" => { arity(2)?; Bool(text(0)?.ends_with(text(1)?)) }
            "match" => {
                arity(2)?;
                let pattern = crate::pattern::Pattern::new(text(1)?, false).map_err(|e| Thrown::Wrong(format!("pattern: {:?}", e)))?;
                Bool(pattern.is_match(text(0)?))
            }
            "push" => { arity(2)?; let List(items) = &args[0] else { return wrong("expected a list") }; let mut items = items.clone(); items.push(args[1].clone()); List(items) }
            "range" => {
                arity(2)?;
                let (Int(a), Int(b)) = (&args[0], &args[1]) else { return wrong("expected two integers") };
                if b.saturating_sub(*a) > 1 << 20 { return wrong("too many"); }
                List((*a..*b).map(Int).collect())
            }
            "keys" => { arity(1)?; let Record(fields) = &args[0] else { return wrong("expected a record") }; List(fields.iter().map(|(n, _)| Value::str(n)).collect()) }
            "is_ok" => { arity(1)?; Bool(matches!(args[0], Value::Ok(_))) }
            "is_err" => { arity(1)?; Bool(matches!(args[0], Value::Err(_))) }
            _ => return wrong("?"),
        })
    })();
    match result { Result::Err(Thrown::Wrong(m)) if m == "?" => None, other => Some(other) }
}
