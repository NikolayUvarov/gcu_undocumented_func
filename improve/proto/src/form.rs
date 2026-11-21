//! The API's forms and their bounds (docs/hub/README.md, 4.2, 5.4, 6.2 and 10.1): a device's registration, a request, an
//! attachment's entry, a message, a state change and a board entry. `check` refuses a form out of its bounds and names
//! the field, so that every refusal says what to change. A form carries data; it grants nothing (MC-11.5).

use crate::state::{Role, State};
use crate::trust::Limits;
use alloc::string::String;
use alloc::vec::Vec;

/// A request's title, in characters.
pub const TITLE_MAX: usize = 120;
/// A request's text and a message's, in bytes of UTF-8.
pub const TEXT_MAX: usize = 8 * 1024;
/// A device's name, a user's display name, a platform, a version and an attachment's name, in characters.
pub const NAME_MAX: usize = 64;
pub const SHORT_MAX: usize = 32;
/// A contact, in characters: an e-mail address, shown to the admin alone.
pub const CONTACT_MAX: usize = 128;
/// A pairing code: 8 characters of PAIR_ALPHABET.
pub const PAIR_LEN: usize = 8;
/// No 0, 1, I, L or O, which read alike.
pub const PAIR_ALPHABET: &str = "23456789ABCDEFGHJKMNPQRSTUVWXYZ";
/// A client's ID for a request, so that a request sent twice is filed once: 8 to 36 of `A-Z a-z 0-9 -`.
pub const CLIENT_ID_MAX: usize = 36;
/// A task's number, `NNN-TRK-MMMM` or a main task's.
pub const TASK_MAX: usize = 24;
/// A state change's reason, in characters.
pub const REASON_MAX: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Empty(&'static str),
    /// Longer than its bound, in its unit.
    TooLong(&'static str, usize),
    /// A control character other than a line end or a tab (in a title, a name: other than none at all).
    Control(&'static str),
    /// Not one of the words or the form the field takes.
    Value(&'static str),
    /// More of them than the band allows.
    Count(&'static str, usize),
    /// Larger than the band allows, in bytes.
    Size(&'static str, u64),
    /// More links than the band allows.
    Links(usize),
}

fn line(field: &'static str, text: &str, max: usize) -> Result<(), Error> {
    if text.trim().is_empty() { return Err(Error::Empty(field)); }
    if text.chars().count() > max { return Err(Error::TooLong(field, max)); }
    if text.chars().any(|c| c.is_control()) { return Err(Error::Control(field)); }
    Ok(())
}

fn body(field: &'static str, text: &str, max: usize) -> Result<(), Error> {
    if text.trim().is_empty() { return Err(Error::Empty(field)); }
    if text.len() > max { return Err(Error::TooLong(field, max)); }
    if text.chars().any(|c| c.is_control() && c != '\n' && c != '\t') { return Err(Error::Control(field)); }
    Ok(())
}

/// Links in a text: each `http://` or `https://`, in any case.
pub fn links(text: &str) -> usize {
    let lower = text.to_ascii_lowercase();
    lower.matches("http://").count() + lower.matches("https://").count()
}

macro_rules! words {
    ($name:ident { $($variant:ident => $word:literal),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name { $($variant),* }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),*];
            pub fn name(self) -> &'static str { match self { $($name::$variant => $word),* } }
            pub fn from_name(word: &str) -> Option<$name> { Self::ALL.iter().copied().find(|v| v.name() == word) }
        }
    };
}

words!(Kind { Fault => "fault", Improvement => "improvement", Capability => "capability" });
words!(Visibility { Public => "public", Private => "private" });
words!(AttachmentKind { Log => "log", Hardware => "hardware", Screenshot => "screenshot", Text => "text" });
words!(DeviceKind { Mind => "mind", Host => "host" });
words!(MessageKind { Comment => "comment", Question => "question", Answer => "answer", Proposal => "proposal", Result => "result", Verdict => "verdict", Status => "status" });
words!(Verdict { Works => "works", Partly => "partly", DoesNotWork => "does-not-work" });
words!(Consent { Accept => "accept", Correct => "correct", Decline => "decline" });

/// How a new device joins: as a new user with a name, or as another device of a user, by a pairing code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Joining { NewUser { display_name: String }, Pair { code: String } }

/// A device's registration (`POST devices`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registration {
    pub public_key: [u8; 32],
    pub name: String,
    pub kind: DeviceKind,
    pub platform: String,
    pub version: String,
    pub joining: Joining,
    pub contact: Option<String>,
    /// The proof of work's counter, when the hub asked for one.
    pub work: Option<u64>,
}

impl Registration {
    pub fn check(&self) -> Result<(), Error> {
        line("name", &self.name, NAME_MAX)?;
        line("platform", &self.platform, SHORT_MAX)?;
        line("version", &self.version, SHORT_MAX)?;
        match &self.joining {
            Joining::NewUser { display_name } => line("display_name", display_name, NAME_MAX)?,
            Joining::Pair { code } => pair_code(code)?,
        }
        if let Some(contact) = &self.contact {
            line("contact", contact, CONTACT_MAX)?;
            // An address, roughly: one @ with something on each side, no space.
            let mut parts = contact.split('@');
            let (Some(a), Some(b), None) = (parts.next(), parts.next(), parts.next()) else { return Err(Error::Value("contact")) };
            if a.is_empty() || b.is_empty() || contact.contains(' ') { return Err(Error::Value("contact")); }
        }
        Ok(())
    }
}

pub fn pair_code(code: &str) -> Result<(), Error> {
    if code.len() != PAIR_LEN || !code.chars().all(|c| PAIR_ALPHABET.contains(c)) { return Err(Error::Value("pair_code")); }
    Ok(())
}

/// An attachment's entry: what it is, its name, its size and its SHA-256; its bytes come by themselves after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment { pub kind: AttachmentKind, pub name: String, pub size: u64, pub sha256: [u8; 32] }

/// A request (`POST requests`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub client_id: String,
    pub kind: Kind,
    pub title: String,
    pub text: String,
    pub visibility: Visibility,
    /// About a vulnerability: private always, sent the way SECURITY.md names, never on the board.
    pub security: bool,
    /// The user's leave for their display name beside the request on the board (6.4); no name without it.
    pub named: bool,
    pub attachments: Vec<Attachment>,
}

impl Request {
    /// Within the bounds of the form and of the user's band.
    pub fn check(&self, limits: &Limits) -> Result<(), Error> {
        client_id(&self.client_id)?;
        line("title", &self.title, TITLE_MAX)?;
        body("text", &self.text, TEXT_MAX)?;
        if self.security && self.visibility != Visibility::Private { return Err(Error::Value("visibility")); }
        if let Some(most) = limits.links {
            let found = links(&self.title) + links(&self.text);
            if found > most as usize { return Err(Error::Links(most as usize)); }
        }
        attachments_within(&self.attachments, limits)
    }
}

/// Attachments within a band's count and size, each named as a file of its own: no path, no leading dot, none twice.
pub fn attachments_within(list: &[Attachment], limits: &Limits) -> Result<(), Error> {
    if list.len() > limits.attachments as usize { return Err(Error::Count("attachments", limits.attachments as usize)); }
    for (i, a) in list.iter().enumerate() {
        line("attachments.name", &a.name, NAME_MAX)?;
        if a.name.contains('/') || a.name.contains('\\') || a.name.starts_with('.') { return Err(Error::Value("attachments.name")); }
        if a.size == 0 { return Err(Error::Empty("attachments.size")); }
        if a.size > limits.attachment_bytes { return Err(Error::Size("attachments.size", limits.attachment_bytes)); }
        if list[..i].iter().any(|b| b.name == a.name) { return Err(Error::Value("attachments.name")); }
    }
    Ok(())
}

pub fn client_id(id: &str) -> Result<(), Error> {
    if !(8..=CLIENT_ID_MAX).contains(&id.len()) || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err(Error::Value("client_id"));
    }
    Ok(())
}

/// A message in a request's thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub kind: MessageKind,
    pub text: String,
    /// The question or the proposal an answer answers, by its number in the thread.
    pub answers: Option<u32>,
    /// An answer to a proposal: it accepts the proposal, corrects it with its text, or declines it (hub-api 3,
    /// 100003-APP-0155). An answer to a question has none; which one `answers` names, the hub knows.
    pub consent: Option<Consent>,
    /// A verdict's word, and the release it was checked on.
    pub verdict: Option<Verdict>,
    pub release: Option<String>,
    /// A log, a screenshot or a text with an answer or a verdict (6.2): their entries here, their bytes after.
    pub attachments: Vec<Attachment>,
}

impl Message {
    /// Within the form's bounds; attachments within the largest band's, which the hub narrows to the user's.
    pub fn check(&self) -> Result<(), Error> {
        // A proposal accepted may come without a word; every other message has a text.
        if !(self.consent == Some(Consent::Accept) && self.text.is_empty()) { body("text", &self.text, TEXT_MAX)?; }
        if self.consent.is_some() && self.kind != MessageKind::Answer { return Err(Error::Value("consent")); }
        match (self.kind, self.answers.is_some(), self.verdict.is_some()) {
            (MessageKind::Answer, true, false) => {}
            (MessageKind::Verdict, false, true) => {}
            (MessageKind::Answer | MessageKind::Verdict, _, _) => return Err(Error::Value("kind")),
            (_, false, false) => {}
            _ => return Err(Error::Value("kind")),
        }
        if self.release.is_some() && self.kind != MessageKind::Verdict && self.kind != MessageKind::Result { return Err(Error::Value("release")); }
        if let Some(release) = &self.release { line("release", release, SHORT_MAX)?; }
        if self.kind == MessageKind::Verdict && self.release.is_none() { return Err(Error::Empty("release")); }
        if !self.attachments.is_empty() && !matches!(self.kind, MessageKind::Answer | MessageKind::Verdict) { return Err(Error::Value("attachments")); }
        attachments_within(&self.attachments, &crate::trust::BANDS[crate::trust::BANDS.len() - 1])
    }
}

/// Whether `role` may post a message of `kind` (docs/hub/README.md, 6.2): the user comments, answers and gives the
/// verdict; an agent comments, asks, proposes and gives the result; the admin comments and asks; the hub alone writes a
/// state's change.
pub fn may_post(role: Role, kind: MessageKind) -> bool {
    use MessageKind::*;
    match role {
        Role::User => matches!(kind, Comment | Answer | Verdict),
        Role::Intake | Role::Track | Role::Integrator => matches!(kind, Comment | Question | Proposal | Result),
        Role::Admin => matches!(kind, Comment | Question),
        Role::Hub => kind == Status,
    }
}

/// A state's change, as the hub records it (MC-10.3): who, when (the hub's clock, ms), from what, to what, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change { pub from: State, pub to: State, pub role: Role, pub at_ms: u64, pub reason: String }

impl Change {
    pub fn check(&self) -> Result<(), Error> {
        line("reason", &self.reason, REASON_MAX)?;
        if !crate::state::allowed(self.from, self.to, self.role) { return Err(Error::Value("to")); }
        Ok(())
    }
}

/// A board entry (docs/hub/README.md, 6.4): no user's name unless they allowed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardEntry {
    pub id: u64,
    pub title: String,
    pub kind: Kind,
    pub track: String,
    pub task: String,
    pub state: State,
    pub since_ms: u64,
    pub release: Option<String>,
    pub author: Option<String>,
}

impl BoardEntry {
    pub fn check(&self) -> Result<(), Error> {
        line("title", &self.title, TITLE_MAX)?;
        if self.track.len() != 3 || !self.track.bytes().all(|b| b.is_ascii_uppercase()) { return Err(Error::Value("track")); }
        line("task", &self.task, TASK_MAX)?;
        if !self.state.on_board() { return Err(Error::Value("state")); }
        if let Some(release) = &self.release { line("release", release, SHORT_MAX)?; }
        if let Some(author) = &self.author { line("author", author, NAME_MAX)?; }
        Ok(())
    }
}
