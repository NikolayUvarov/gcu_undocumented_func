//! The system clipboard's client (000-APP-0032, `idl/clipboard.wit` 1.0): a text copied or pasted in parts, which these
//! calls join, through the client in SLOT_CLIPBOARD that the shell lends for REQUEST_CLIPBOARD. Without one a program
//! keeps its own copy buffer, as before.
use crate::abi::{CAP_KIND_ENDPOINT, SLOT_CLIPBOARD};
use crate::idl::clipboard as api;
use crate::idl::wire;
use crate::ipc::Endpoint;
use alloc::string::String;

/// A manager's client: the shell, and the window manager it started; it names the program in front.
pub const BADGE_MANAGER: u16 = 1;
/// A program's client, as the shell lends it.
pub const BADGE_PROGRAM: u16 = 2;
/// The longest text, in bytes.
pub const MAX: usize = 65536;
const PART: usize = 4000;
// Each part is a message; the service answers at once.
const CALL_MS: u32 = 1000;

/// Why a copy or a paste did not happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { NoClient, Behind, Refused, Invalid, Changed, TooLong, NoAnswer }

impl From<api::Error> for Error {
    fn from(e: api::Error) -> Self { match e { api::Error::Behind => Error::Behind, api::Error::Refused => Error::Refused, api::Error::Invalid => Error::Invalid, api::Error::Changed => Error::Changed } }
}

impl Error {
    /// The line a program shows.
    pub fn text(self) -> &'static str {
        match self {
            Error::NoClient => "NO CLIPBOARD (THE SHELL LENDS IT FOR REQUEST_CLIPBOARD)",
            Error::Behind => "THE CLIPBOARD ANSWERS THE PROGRAM IN FRONT ONLY",
            Error::Refused => "THE CLIPBOARD REFUSED",
            Error::Invalid => "THE CLIPBOARD TOOK IT AS INVALID",
            Error::Changed => "THE CLIPBOARD CHANGED WHILE IT WAS READ",
            Error::TooLong => "LONGER THAN THE CLIPBOARD HOLDS (64 KIB)",
            Error::NoAnswer => "THE CLIPBOARD DID NOT ANSWER",
        }
    }
}

/// The client this program holds, as the shell lends it; None without one.
pub fn client() -> Option<Endpoint> { (crate::dev::cap_info(SLOT_CLIPBOARD).0 == CAP_KIND_ENDPOINT).then_some(Endpoint(SLOT_CLIPBOARD)) }

fn call<T>(f: impl FnOnce() -> crate::sys::Result<core::result::Result<T, api::Error>>) -> Result<T, Error> {
    match wire::with_timeout(CALL_MS, f) { Ok(Ok(value)) => Ok(value), Ok(Err(e)) => Err(e.into()), Err(_) => Err(Error::NoAnswer) }
}

/// `text` to the clipboard through `client`: its new generation.
pub fn copy_through(client: Endpoint, text: &str) -> Result<u32, Error> {
    if text.len() > MAX { return Err(Error::TooLong); }
    let mut at = 0;
    loop {
        let mut end = (at + PART).min(text.len());
        while !text.is_char_boundary(end) { end -= 1; }
        let last = end == text.len();
        let generation = call(|| api::set_part(client, at as u32, &text[at..end], last))?;
        if last { return Ok(generation); }
        at = end;
    }
}

/// The clipboard's text through `client`, read again from its start when it is set meanwhile (three tries).
pub fn paste_through(client: Endpoint) -> Result<String, Error> {
    'tries: for _ in 0..3 {
        let first = call(|| api::get_part(client, 0, 0))?;
        let (generation, length) = (first.generation, first.length as usize);
        let mut text = String::with_capacity(length);
        text.push_str(first.text.as_str());
        while text.len() < length {
            match call(|| api::get_part(client, generation, text.len() as u32)) {
                Ok(part) if !part.text.as_str().is_empty() => text.push_str(part.text.as_str()),
                Ok(_) => return Err(Error::Invalid),
                Err(Error::Changed) => continue 'tries,
                Err(e) => return Err(e),
            }
        }
        return Ok(text);
    }
    Err(Error::Changed)
}

/// `text` to the clipboard, through the client the shell lent.
pub fn copy(text: &str) -> Result<u32, Error> { copy_through(client().ok_or(Error::NoClient)?, text) }

/// The clipboard's text, through the client the shell lent.
pub fn paste() -> Result<String, Error> { paste_through(client().ok_or(Error::NoClient)?) }

/// A manager's word: `pid` is the program in front (0: none).
pub fn front(manager: Endpoint, pid: u64) -> Result<(), Error> { call(|| api::front(manager, pid)) }
