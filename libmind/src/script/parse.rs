//! The parser of `msh` (issue 094): source text to a `Script`, in one pass over the characters. Expressions are
//! read as tokens; a line that starts with a name not followed by `(` or `=` is a command, read as words.
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

/// Where something is in the source: line and column, from 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct At { pub line: u32, pub column: u32 }

/// Why the source is not a script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError { pub at: At, pub message: String }

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { write!(f, "line {}, column {}: {}", self.at.line, self.at.column, self.message) }
}

/// A piece of a string or a command word: text as written, or an expression in braces.
#[derive(Clone, Debug, PartialEq)]
pub enum Part { Text(String), Expr(Expr) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op { Add, Sub, Mul, Div, Rem, Eq, Ne, Lt, Le, Gt, Ge, Not, Neg }

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Nil,
    Bool(bool),
    Int(i64),
    Str(Vec<Part>),
    Var(String, At),
    List(Vec<Expr>),
    Record(Vec<(String, Expr)>),
    Unary(Op, Box<Expr>),
    Binary(Op, Box<Expr>, Box<Expr>, At),
    And(Box<Expr>, Box<Expr>),
    OrElse(Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>, At),
    Field(Box<Expr>, String, At),
    Index(Box<Expr>, Box<Expr>, At),
    /// `expr?`: the value of `ok(v)`; `err(r)` fails.
    Propagate(Box<Expr>, At),
    /// `expr or { … }` / `expr or other`: the handler runs when `expr` fails or is `err(r)` (`error` is `r` there).
    Handle(Box<Expr>, Handler),
    /// A command line: its words.
    Command(Vec<Vec<Part>>, At),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Handler { Block(Vec<Stmt>), Expr(Box<Expr>) }

#[derive(Clone, Debug, PartialEq)]
pub struct Stmt { pub kind: Kind, pub at: At }

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Let(String, Expr),
    Assign(String, Expr),
    If(Vec<(Expr, Vec<Stmt>)>, Option<Vec<Stmt>>),
    While(Expr, Vec<Stmt>),
    For(String, Expr, Vec<Stmt>),
    Break,
    Continue,
    Return(Option<Expr>),
    Try(Vec<Stmt>, String, Vec<Stmt>),
    Expr(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function { pub name: String, pub params: Vec<String>, pub body: Vec<Stmt>, pub at: At }

/// A parsed script: what it declares it needs (`requires:`), its statements, its functions.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Script { pub requires: Vec<String>, pub body: Vec<Stmt>, pub functions: Vec<Function> }

const KEYWORDS: [&str; 15] = ["let", "if", "else", "while", "for", "in", "fn", "return", "break", "continue", "try", "catch", "true", "false", "nil"];
const MAX_DEPTH: usize = 64;

pub fn is_keyword(word: &str) -> bool { KEYWORDS.contains(&word) || word == "or" }

/// Parses `source` (a file, or a line typed at the prompt).
pub fn parse(source: &str) -> Result<Script, ParseError> {
    let mut parser = Parser { src: source, at: 0, line: 1, column: 1, functions: Vec::new(), depth: 0 };
    let requires = parser.header();
    let body = parser.block(false)?;
    Ok(Script { requires, body, functions: parser.functions })
}

struct Parser<'a> { src: &'a str, at: usize, line: u32, column: u32, functions: Vec<Function>, depth: usize }

impl<'a> Parser<'a> {
    fn here(&self) -> At { At { line: self.line, column: self.column } }
    fn fail<T>(&self, message: &str) -> Result<T, ParseError> { Err(ParseError { at: self.here(), message: String::from(message) }) }
    fn peek(&self) -> Option<char> { self.src[self.at..].chars().next() }
    fn peek2(&self) -> Option<char> { self.src[self.at..].chars().nth(1) }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += c.len_utf8();
        if c == '\n' { self.line += 1; self.column = 1; } else { self.column += 1; }
        Some(c)
    }
    fn eat(&mut self, c: char) -> bool { if self.peek() == Some(c) { self.bump(); true } else { false } }
    // Spaces and tabs (and comments); newlines too when `lines`.
    fn skip(&mut self, lines: bool) {
        loop {
            match self.peek() {
                Some(' ' | '\t' | '\r') => { self.bump(); }
                Some('\n') if lines => { self.bump(); }
                Some('#') => { while self.peek().is_some_and(|c| c != '\n') { self.bump(); } }
                _ => return,
            }
        }
    }
    fn expect(&mut self, c: char, lines: bool) -> Result<(), ParseError> {
        self.skip(lines);
        if self.eat(c) { Ok(()) } else { self.fail(&alloc::format!("expected '{}'", c)) }
    }
    fn word_char(c: char) -> bool { c.is_alphanumeric() || c == '_' }
    fn ident(&mut self) -> Option<String> {
        let start = self.at;
        if !self.peek().is_some_and(|c| c.is_alphabetic() || c == '_') { return None; }
        while self.peek().is_some_and(Self::word_char) { self.bump(); }
        Some(String::from(&self.src[start..self.at]))
    }
    // The next name without consuming it.
    fn peek_ident(&self) -> Option<&'a str> {
        let rest = &self.src[self.at..];
        let end = rest.char_indices().find(|&(_, c)| !Self::word_char(c)).map_or(rest.len(), |(i, _)| i);
        (end > 0 && rest.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')).then(|| &rest[..end])
    }
    fn keyword(&mut self, word: &str) -> bool {
        if self.peek_ident() == Some(word) { for _ in word.chars() { self.bump(); } true } else { false }
    }
    fn name(&mut self, what: &str) -> Result<String, ParseError> {
        self.skip(false);
        let at = self.here();
        match self.ident() {
            Some(name) if !is_keyword(&name) => Ok(name),
            _ => Err(ParseError { at, message: alloc::format!("expected {}", what) }),
        }
    }

    // `#!msh` and comments, then `requires: words` on the first line with something on it.
    fn header(&mut self) -> Vec<String> {
        self.skip(true);
        let rest = &self.src[self.at..];
        if !rest.starts_with("requires:") { return Vec::new(); }
        let line = rest.lines().next().unwrap_or("");
        let words = line["requires:".len()..].split('#').next().unwrap_or("").split_whitespace().map(String::from).collect();
        for _ in line.chars() { self.bump(); }
        words
    }

    // Statements until `}` (when `braced`) or the end.
    fn block(&mut self, braced: bool) -> Result<Vec<Stmt>, ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH { return self.fail("nested too deep"); }
        let mut body = Vec::new();
        loop {
            self.skip(true);
            while self.eat(';') { self.skip(true); }
            match self.peek() {
                None if braced => return self.fail("expected '}'"),
                None => break,
                Some('}') if braced => { self.bump(); break; }
                Some('}') => return self.fail("unexpected '}'"),
                _ => {}
            }
            if let Some(stmt) = self.statement()? { body.push(stmt); }
            // A statement ends at a newline, ';', '}' or the end.
            self.skip(false);
            match self.peek() { None | Some('\n' | ';' | '}') => {} _ => return self.fail("expected the end of the line") }
        }
        self.depth -= 1;
        Ok(body)
    }
    fn braced(&mut self) -> Result<Vec<Stmt>, ParseError> { self.expect('{', true)?; self.block(true) }

    fn statement(&mut self) -> Result<Option<Stmt>, ParseError> {
        let at = self.here();
        let stmt = |kind| Ok(Some(Stmt { kind, at }));
        let word = self.peek_ident();
        match word {
            Some("let") => {
                self.keyword("let");
                let name = self.name("a name")?;
                self.expect('=', false)?;
                let value = self.expr()?;
                stmt(Kind::Let(name, value))
            }
            Some("if") => {
                let mut arms = Vec::new();
                let mut otherwise = None;
                self.keyword("if");
                loop {
                    let cond = self.expr()?;
                    arms.push((cond, self.braced()?));
                    // `else` after the `}`, on its line or the next.
                    let mark = (self.at, self.line, self.column);
                    self.skip(true);
                    if !self.keyword("else") { (self.at, self.line, self.column) = mark; break; }
                    self.skip(false);
                    if self.keyword("if") { continue; }
                    otherwise = Some(self.braced()?);
                    break;
                }
                stmt(Kind::If(arms, otherwise))
            }
            Some("while") => { self.keyword("while"); let cond = self.expr()?; stmt(Kind::While(cond, self.braced()?)) }
            Some("for") => {
                self.keyword("for");
                let name = self.name("a name")?;
                self.skip(false);
                if !self.keyword("in") { return self.fail("expected 'in'"); }
                let list = self.expr()?;
                stmt(Kind::For(name, list, self.braced()?))
            }
            Some("fn") => {
                self.keyword("fn");
                let name = self.name("a function name")?;
                self.expect('(', false)?;
                let mut params = Vec::new();
                self.skip(true);
                if !self.eat(')') {
                    loop {
                        params.push(self.name("a parameter")?);
                        self.skip(true);
                        if self.eat(')') { break; }
                        self.expect(',', true)?;
                    }
                }
                if self.functions.iter().any(|f| f.name == name) { return Err(ParseError { at, message: alloc::format!("function {} defined twice", name) }); }
                let body = self.braced()?;
                self.functions.push(Function { name, params, body, at });
                Ok(None)
            }
            Some("return") => {
                self.keyword("return");
                self.skip(false);
                let value = match self.peek() { None | Some('\n' | ';' | '}') => None, _ => Some(self.expr()?) };
                stmt(Kind::Return(value))
            }
            Some("break") => { self.keyword("break"); stmt(Kind::Break) }
            Some("continue") => { self.keyword("continue"); stmt(Kind::Continue) }
            Some("try") => {
                self.keyword("try");
                let body = self.braced()?;
                self.skip(true);
                if !self.keyword("catch") { return self.fail("expected 'catch'"); }
                let name = self.name("a name for the error")?;
                stmt(Kind::Try(body, name, self.braced()?))
            }
            Some("requires") if self.src[self.at + 8..].starts_with(':') => self.fail("requires: must come before the statements"),
            Some(name) if !is_keyword(name) => {
                // `name = value`, `name(…)` and the expressions that start with them; otherwise a command line.
                let after = self.src[self.at + name.len()..].trim_start_matches([' ', '\t']);
                let assign = after.starts_with('=') && !after.starts_with("==");
                if assign {
                    let name = self.name("a name")?;
                    self.expect('=', false)?;
                    let value = self.expr()?;
                    return stmt(Kind::Assign(name, value));
                }
                if self.src[self.at + name.len()..].starts_with('(') { return stmt(Kind::Expr(self.expr()?)); }
                let command = self.command()?;
                stmt(Kind::Expr(command))
            }
            _ => stmt(Kind::Expr(self.expr()?)),
        }
    }

    // A command line: words until the end of the line, ';' or '}'; `or { … }` / `or expr` handles its failure and a
    // last word `?` says what is meant anyway (a failed command stops the script).
    fn command(&mut self) -> Result<Expr, ParseError> {
        let at = self.here();
        let mut words: Vec<Vec<Part>> = Vec::new();
        loop {
            self.skip(false);
            match self.peek() {
                None | Some('\n' | ';' | '}') => break,
                _ => {}
            }
            if self.peek_ident() == Some("or") && words.len() >= 1 {
                let rest = self.src[self.at + 2..].trim_start_matches([' ', '\t']);
                if rest.starts_with('{') || (!rest.is_empty() && !rest.starts_with(['\n', ';', '}'])) {
                    self.keyword("or");
                    let command = Expr::Command(words, at);
                    return Ok(Expr::Handle(Box::new(command), self.handler()?));
                }
            }
            if self.peek() == Some('?') && matches!(self.peek2(), None | Some(' ' | '\t' | '\r' | '\n' | ';' | '}')) {
                self.bump();
                continue;
            }
            words.push(self.word()?);
        }
        Ok(Expr::Command(words, at))
    }

    // One word of a command: "quoted", or bare with `{expr}` inside.
    fn word(&mut self) -> Result<Vec<Part>, ParseError> {
        if self.peek() == Some('"') { return self.string(); }
        let mut parts = Vec::new();
        let mut text = String::new();
        while let Some(c) = self.peek() {
            match c {
                ' ' | '\t' | '\r' | '\n' | ';' | '}' => break,
                '{' => {
                    self.bump();
                    if !text.is_empty() { parts.push(Part::Text(core::mem::take(&mut text))); }
                    parts.push(Part::Expr(self.expr()?));
                    self.expect('}', true)?;
                }
                '\\' => { self.bump(); if let Some(c) = self.bump() { text.push(c); } }
                _ => { text.push(c); self.bump(); }
            }
        }
        if !text.is_empty() || parts.is_empty() { parts.push(Part::Text(text)); }
        Ok(parts)
    }

    // "text {expr} \n \t \" \\ \{"
    fn string(&mut self) -> Result<Vec<Part>, ParseError> {
        let start = self.here();
        self.bump();
        let mut parts = Vec::new();
        let mut text = String::new();
        loop {
            match self.bump() {
                None => return Err(ParseError { at: start, message: String::from("unclosed string") }),
                Some('"') => break,
                Some('\\') => match self.bump() {
                    Some('n') => text.push('\n'),
                    Some('t') => text.push('\t'),
                    Some(c @ ('"' | '\\' | '{' | '}')) => text.push(c),
                    _ => return self.fail("unknown escape (\\n \\t \\\" \\\\ \\{ \\})"),
                },
                Some('{') => {
                    if !text.is_empty() { parts.push(Part::Text(core::mem::take(&mut text))); }
                    parts.push(Part::Expr(self.expr()?));
                    self.expect('}', true)?;
                }
                Some(c) => text.push(c),
            }
        }
        if !text.is_empty() || parts.is_empty() { parts.push(Part::Text(text)); }
        Ok(parts)
    }

    fn handler(&mut self) -> Result<Handler, ParseError> {
        self.skip(false);
        if self.peek() == Some('{') { Ok(Handler::Block(self.braced()?)) } else { Ok(Handler::Expr(Box::new(self.logic_or()?))) }
    }

    pub fn expr(&mut self) -> Result<Expr, ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH { return self.fail("nested too deep"); }
        let value = self.logic_or()?;
        self.skip(false);
        let value = if self.peek_ident() == Some("or") { self.keyword("or"); Expr::Handle(Box::new(value), self.handler()?) } else { value };
        self.depth -= 1;
        Ok(value)
    }
    fn logic_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.logic_and()?;
        loop {
            self.skip(false);
            if self.src[self.at..].starts_with("||") { self.bump(); self.bump(); let right = self.logic_and()?; left = Expr::OrElse(Box::new(left), Box::new(right)); } else { return Ok(left); }
        }
    }
    fn logic_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.compare()?;
        loop {
            self.skip(false);
            if self.src[self.at..].starts_with("&&") { self.bump(); self.bump(); let right = self.compare()?; left = Expr::And(Box::new(left), Box::new(right)); } else { return Ok(left); }
        }
    }
    fn compare(&mut self) -> Result<Expr, ParseError> {
        let left = self.sum()?;
        self.skip(false);
        let at = self.here();
        let rest = &self.src[self.at..];
        let (op, len) = if rest.starts_with("==") { (Op::Eq, 2) } else if rest.starts_with("!=") { (Op::Ne, 2) } else if rest.starts_with("<=") { (Op::Le, 2) }
                        else if rest.starts_with(">=") { (Op::Ge, 2) } else if rest.starts_with('<') { (Op::Lt, 1) } else if rest.starts_with('>') { (Op::Gt, 1) } else { return Ok(left) };
        for _ in 0..len { self.bump(); }
        let right = self.sum()?;
        Ok(Expr::Binary(op, Box::new(left), Box::new(right), at))
    }
    fn sum(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.product()?;
        loop {
            self.skip(false);
            let at = self.here();
            let op = match self.peek() { Some('+') => Op::Add, Some('-') => Op::Sub, _ => return Ok(left) };
            self.bump();
            let right = self.product()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right), at);
        }
    }
    fn product(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.unary()?;
        loop {
            self.skip(false);
            let at = self.here();
            let op = match self.peek() { Some('*') => Op::Mul, Some('/') => Op::Div, Some('%') => Op::Rem, _ => return Ok(left) };
            self.bump();
            let right = self.unary()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right), at);
        }
    }
    fn unary(&mut self) -> Result<Expr, ParseError> {
        self.skip(false);
        match self.peek() {
            Some('!') if self.peek2() != Some('=') => { self.bump(); Ok(Expr::Unary(Op::Not, Box::new(self.unary()?))) }
            Some('-') => { self.bump(); Ok(Expr::Unary(Op::Neg, Box::new(self.unary()?))) }
            _ => self.postfix(),
        }
    }
    fn postfix(&mut self) -> Result<Expr, ParseError> {
        let mut value = self.primary()?;
        loop {
            let at = self.here();
            match self.peek() {
                Some('.') => { self.bump(); let name = self.ident().ok_or_else(|| ParseError { at, message: String::from("expected a field name") })?; value = Expr::Field(Box::new(value), name, at); }
                Some('[') => { self.bump(); let index = self.expr()?; self.expect(']', true)?; value = Expr::Index(Box::new(value), Box::new(index), at); }
                Some('?') => { self.bump(); value = Expr::Propagate(Box::new(value), at); }
                _ => return Ok(value),
            }
        }
    }
    fn primary(&mut self) -> Result<Expr, ParseError> {
        self.skip(false);
        let at = self.here();
        match self.peek() {
            Some('"') => Ok(Expr::Str(self.string()?)),
            Some('(') => { self.bump(); let value = self.expr()?; self.expect(')', true)?; Ok(value) }
            Some('[') => {
                self.bump();
                let mut items = Vec::new();
                self.skip(true);
                if !self.eat(']') {
                    loop {
                        items.push(self.expr()?);
                        self.skip(true);
                        if self.eat(']') { break; }
                        self.expect(',', true)?;
                        self.skip(true);
                        if self.eat(']') { break; } // a trailing comma
                    }
                }
                Ok(Expr::List(items))
            }
            Some('{') => {
                self.bump();
                let mut fields = Vec::new();
                self.skip(true);
                if !self.eat('}') {
                    loop {
                        self.skip(true);
                        let name = self.name("a field name")?;
                        self.expect(':', true)?;
                        fields.push((name, self.expr()?));
                        self.skip(true);
                        if self.eat('}') { break; }
                        self.expect(',', true)?;
                        self.skip(true);
                        if self.eat('}') { break; }
                    }
                }
                Ok(Expr::Record(fields))
            }
            Some(c) if c.is_ascii_digit() => {
                let start = self.at;
                while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') { self.bump(); }
                let digits: String = self.src[start..self.at].chars().filter(|&c| c != '_').collect();
                digits.parse().map(Expr::Int).map_err(|_| ParseError { at, message: String::from("number too large") })
            }
            Some(c) if c.is_alphabetic() || c == '_' => {
                let name = self.ident().unwrap_or_default();
                match name.as_str() {
                    "true" => return Ok(Expr::Bool(true)),
                    "false" => return Ok(Expr::Bool(false)),
                    "nil" => return Ok(Expr::Nil),
                    _ if is_keyword(&name) => return Err(ParseError { at, message: alloc::format!("unexpected '{}'", name) }),
                    _ => {}
                }
                if self.eat('(') {
                    let mut args = Vec::new();
                    self.skip(true);
                    if !self.eat(')') {
                        loop {
                            args.push(self.expr()?);
                            self.skip(true);
                            if self.eat(')') { break; }
                            self.expect(',', true)?;
                        }
                    }
                    return Ok(Expr::Call(name, args, at));
                }
                Ok(Expr::Var(name, at))
            }
            Some(c) => Err(ParseError { at, message: alloc::format!("unexpected '{}'", c) }),
            None => Err(ParseError { at, message: String::from("unexpected end") }),
        }
    }
}
