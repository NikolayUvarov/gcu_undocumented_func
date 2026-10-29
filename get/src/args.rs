// What `get` was asked (000-NET-0048): its words, split as a shell splits them, read into options; and the exit
// statuses it ends with, which follow curl's for the common cases. tests/get_host.rs checks both on the host.
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// The exit statuses (curl's numbers where curl has one).
pub const OK: i32 = 0;
pub const UNSUPPORTED: i32 = 1; // a scheme other than https, or http without --insecure-http
pub const USAGE: i32 = 2;
pub const BAD_URL: i32 = 3;
pub const NO_HOST: i32 = 6;
pub const NO_CONNECTION: i32 = 7;
pub const HTTP_ERROR: i32 = 22; // a status of 400 or more, with -f
pub const WRITE_ERROR: i32 = 23;
pub const READ_ERROR: i32 = 26; // a file to send could not be read
pub const TIMEOUT: i32 = 28;
pub const TLS_ERROR: i32 = 35;
pub const TOO_MANY_REDIRECTS: i32 = 47;
pub const RECEIVE_ERROR: i32 = 56;
pub const CERTIFICATE: i32 = 60;

pub const HEADERS_MAX: usize = 14; // the library sends 16 at most: Range and Content-Type may join them
pub const FORM_MAX: usize = 8;
/// Redirects followed at most (the HTTP library's own bound).
pub const REDIRECTS_MAX: u32 = 10;

#[derive(Debug, PartialEq, Clone)]
pub enum Data { Text(String), File(String) }

#[derive(Debug, PartialEq, Clone)]
pub enum Form { Field { name: String, value: String }, File { name: String, path: String } }

#[derive(Debug, PartialEq, Clone)]
pub enum Output { Console, File(String), Remote }

#[derive(Debug, PartialEq, Clone)]
pub struct Options {
    pub url: String,
    pub method: Option<String>,
    pub headers: Vec<(String, String)>,
    pub data: Option<Data>,
    pub form: Vec<Form>,
    pub output: Output,
    pub resume: bool,
    pub head: bool,
    pub follow: bool,
    pub max_redirs: u32,
    pub silent: bool,
    pub write_out: bool,
    pub max_time: Option<u32>,
    pub fail: bool,
    pub insecure_http: bool,
}

impl Options {
    /// The method to send: -X's, else POST with a body, else GET.
    pub fn method(&self) -> &str {
        match &self.method { Some(m) => m, None if self.data.is_some() || !self.form.is_empty() => "POST", None => "GET" }
    }
}

/// The words of `text`: split at spaces outside quotes; '…' keeps everything, "…" and a bare word take `\` before the
/// next character. An unclosed quote is refused.
pub fn split(text: &str) -> Result<Vec<String>, String> {
    let (mut words, mut word, mut started) = (Vec::new(), String::new(), false);
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\n' => { if started { words.push(core::mem::take(&mut word)); started = false; } }
            '\'' => {
                started = true;
                loop { match chars.next() { Some('\'') => break, Some(c) => word.push(c), None => return Err("an unclosed '".to_string()) } }
            }
            '"' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() { Some(c) => word.push(c), None => return Err("an unclosed \"".to_string()) },
                        Some(c) => word.push(c),
                        None => return Err("an unclosed \"".to_string()),
                    }
                }
            }
            '\\' => { started = true; if let Some(c) = chars.next() { word.push(c); } }
            c => { started = true; word.push(c); }
        }
    }
    if started { words.push(word); }
    Ok(words)
}

fn header(text: &str) -> Result<(String, String), String> {
    let (name, value) = text.split_once(':').ok_or_else(|| alloc::format!("-H {:?}: not 'Name: value'", text))?;
    let name = name.trim();
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)) {
        return Err(alloc::format!("-H {:?}: not a header name", text));
    }
    Ok((name.to_string(), value.trim().to_string()))
}

fn form(text: &str) -> Result<Form, String> {
    let (name, value) = text.split_once('=').filter(|(n, _)| !n.is_empty()).ok_or_else(|| alloc::format!("-F {:?}: not name=value or name=@file", text))?;
    Ok(match value.strip_prefix('@') {
        Some(path) if !path.is_empty() => Form::File { name: name.to_string(), path: path.to_string() },
        _ => Form::Field { name: name.to_string(), value: value.to_string() },
    })
}

/// The options of `words` (the program's name not among them).
pub fn parse(words: &[String]) -> Result<Options, String> {
    let mut o = Options { url: String::new(), method: None, headers: Vec::new(), data: None, form: Vec::new(), output: Output::Console, resume: false,
                          head: false, follow: true, max_redirs: 10, silent: false, write_out: false, max_time: None, fail: false, insecure_http: false };
    let mut url = None;
    let mut i = 0;
    while i < words.len() {
        let word = words[i].as_str();
        let mut value = || -> Result<&str, String> { i += 1; words.get(i).map(String::as_str).ok_or_else(|| alloc::format!("{} needs a value", word)) };
        match word {
            "-o" | "--output" => o.output = Output::File(value()?.to_string()),
            "-O" | "--remote-name" => o.output = Output::Remote,
            "-c" | "--continue" => o.resume = true,
            "-I" | "--head" => o.head = true,
            "-X" | "--request" => {
                let m = value()?.to_ascii_uppercase();
                if !matches!(m.as_str(), "GET" | "POST" | "PUT") { return Err(alloc::format!("-X {}: GET, POST or PUT", m)); }
                o.method = Some(m);
            }
            "-H" | "--header" => { if o.headers.len() == HEADERS_MAX { return Err("more than 14 headers".to_string()); } let h = header(value()?)?; o.headers.push(h); }
            "-d" | "--data" => {
                let v = value()?;
                o.data = Some(match v.strip_prefix('@') { Some(path) if !path.is_empty() => Data::File(path.to_string()), _ => Data::Text(v.to_string()) });
            }
            "-F" | "--form" => { if o.form.len() == FORM_MAX { return Err("more than 8 form parts".to_string()); } let f = form(value()?)?; o.form.push(f); }
            "-L" | "--location" => o.follow = true,
            "--no-location" => o.follow = false,
            "--max-redirs" => o.max_redirs = value()?.parse().ok().filter(|&n: &u32| n <= REDIRECTS_MAX).ok_or_else(|| "--max-redirs takes 0 to 10".to_string())?,
            "-s" | "--silent" => o.silent = true,
            "-w" | "--write-out" => o.write_out = true,
            "-f" | "--fail" => o.fail = true,
            "--max-time" | "-m" => o.max_time = Some(value()?.parse().ok().filter(|&s: &u32| s > 0).ok_or_else(|| "--max-time takes seconds, 1 or more".to_string())?),
            "--insecure-http" => o.insecure_http = true,
            w if w.starts_with('-') && w.len() > 1 => return Err(alloc::format!("{}: no such option", w)),
            w => { if url.replace(w.to_string()).is_some() { return Err("one URL only".to_string()); } }
        }
        i += 1;
    }
    o.url = url.ok_or_else(|| "no URL".to_string())?;
    if o.data.is_some() && !o.form.is_empty() { return Err("-d and -F do not go together".to_string()); }
    if o.head && (o.data.is_some() || !o.form.is_empty()) { return Err("-I sends no body".to_string()); }
    if o.resume && o.output == Output::Console { return Err("-c needs -o or -O".to_string()); }
    Ok(o)
}

/// The file `-O` saves to: the URL path's last segment, without its query; None when that is empty or not a plain name.
pub fn remote_name(path: &str) -> Option<&str> {
    let path = path.split(['?', '#']).next().unwrap_or("");
    let name = path.rsplit('/').next().unwrap_or("");
    (!name.is_empty() && name != "." && name != ".." && name.bytes().all(|b| b.is_ascii_graphic() && !b"\\:*\"<>|".contains(&b))).then_some(name)
}
