//! HTTP/1.1 enough for effector485: a request read whole (Connection: close), an answer, and the client's call.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// The largest body taken: an operation's request is small.
const BODY_MAX: usize = 1 << 20;

pub struct Request { pub method: String, pub path: String, pub query: Vec<(String, String)>, pub key: Option<String>, pub body: String }

impl Request {
    pub fn param(&self, name: &str) -> Option<&str> { self.query.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str()) }
}

fn unescape(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => { out.push(b' '); i += 1; }
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) { Ok(v) => { out.push(v); i += 3; } Err(_) => { out.push(b'%'); i += 1; } }
            }
            c => { out.push(c); i += 1; }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn read_request(stream: &mut TcpStream) -> Result<Request, String> {
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("/").to_string());
    let (mut length, mut key) = (0usize, None);
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).map_err(|e| e.to_string())? == 0 || h.trim().is_empty() { break; }
        let Some((name, value)) = h.split_once(':') else { continue };
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => length = value.trim().parse().map_err(|_| "a bad Content-Length")?,
            "x-api-key" => key = Some(value.trim().to_string()),
            _ => {}
        }
    }
    if length > BODY_MAX { return Err("the body is too large".into()); }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).map_err(|e| e.to_string())?;
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let query = query.split('&').filter(|p| !p.is_empty()).map(|p| {
        let (k, v) = p.split_once('=').unwrap_or((p, ""));
        (unescape(k), unescape(v))
    }).collect();
    Ok(Request { method, path: path.to_string(), query, key, body: String::from_utf8(body).map_err(|_| "the body is not UTF-8")? })
}

pub fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &str) {
    let reason = match status { 200 => "OK", 201 => "Created", 400 => "Bad Request", 401 => "Unauthorized", 404 => "Not Found", 409 => "Conflict", _ => "Error" };
    let head = format!("HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    stream.write_all(head.as_bytes()).and_then(|_| stream.write_all(body.as_bytes())).ok();
}

/// One call to the daemon at `at`: its status and body.
pub fn call(at: &str, method: &str, path: &str, key: Option<&str>, body: Option<&str>) -> Result<(u16, String), String> {
    let mut stream = TcpStream::connect(at).map_err(|e| format!("{at}: {e} (is `effector485 daemon` running?)"))?;
    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
    let body = body.unwrap_or("");
    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {at}\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    if !body.is_empty() { request.push_str("Content-Type: application/json\r\n"); }
    if let Some(k) = key { request.push_str(&format!("X-API-Key: {k}\r\n")); }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).and_then(|_| stream.write_all(body.as_bytes())).map_err(|e| e.to_string())?;
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&answer);
    let (head, body) = text.split_once("\r\n\r\n").ok_or("the daemon's answer has no head")?;
    let status = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or("the daemon's answer has no status")?;
    Ok((status, body.to_string()))
}

/// A query parameter's value made safe in a URL.
pub fn escape(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}
