//! A user's trust, from −1023 to +1023, and what it allows (docs/hub/README.md, section 5). Bands of 128 from 0: each doubles
//! the requests a day and widens the rest. No new request or message from a negative trust; −1023 is a ban. The hub
//! changes trust by itself only within −256 … +512; beyond them only the admin sets it.

pub const MIN: i16 = -1023;
pub const MAX: i16 = 1023;
/// The bounds of the hub's own changes (the maintainer's choice of 2026-11-20).
pub const AUTO_MIN: i16 = -256;
pub const AUTO_MAX: i16 = 512;
/// The longest reason the admin gives for setting a trust.
pub const REASON_MAX: usize = 200;

/// A user's trust, always within MIN … MAX.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Trust(i16);

impl Trust {
    /// None outside −1023 … +1023.
    pub fn new(value: i16) -> Option<Trust> { (MIN..=MAX).contains(&value).then_some(Trust(value)) }
    pub fn value(self) -> i16 { self.0 }
    /// The band, 0 to 7, for a trust of 0 or more; None for a negative one.
    pub fn band(self) -> Option<u8> { (self.0 >= 0).then(|| (self.0 / 128) as u8) }

    /// The trust after one of the hub's own changes: a raise never takes it above AUTO_MAX, nor a cut below AUTO_MIN, and
    /// neither undoes what the admin set beyond them.
    pub fn after(self, event: Event) -> Trust {
        let old = self.0;
        let delta = event.delta();
        let new = if delta > 0 { (old + delta).min(old.max(AUTO_MAX)) } else { (old + delta).max(old.min(AUTO_MIN)) };
        Trust(new.clamp(MIN, MAX))
    }
}

/// What changes a trust by itself (docs/hub/README.md, 5.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// A request of the user's accepted as a task.
    Accepted,
    /// A request done and verified by the user.
    Verified,
    /// A request rejected as not a request.
    NotARequest,
    /// A request rejected as spam, by the admin.
    Spam,
}

impl Event {
    pub fn delta(self) -> i16 { match self { Event::Accepted => 32, Event::Verified => 16, Event::NotARequest => -16, Event::Spam => -256 } }
    pub fn name(self) -> &'static str {
        match self { Event::Accepted => "accepted", Event::Verified => "verified", Event::NotARequest => "not-a-request", Event::Spam => "spam" }
    }
}

/// The update channels a band reaches: `stable` for all, `test` from band 2, `dev` from band 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channels { pub test: bool, pub dev: bool }

impl Channels {
    /// Whether the channel `name` is served; an unknown name is not.
    pub fn allows(self, name: &str) -> bool {
        match name { "stable" => true, "test" => self.test, "dev" => self.dev, _ => false }
    }
}

/// What a user may send, by band (docs/hub/README.md, 5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// New requests a day, and the bucket's size.
    pub requests_per_day: u32,
    pub burst: u32,
    /// Messages a day (comments, answers beyond the one a question is owed, verdicts).
    pub messages_per_day: u32,
    /// Attachments to a request, and the largest one's bytes.
    pub attachments: u8,
    pub attachment_bytes: u64,
    /// Links in a request's text; None: no bound but the text's.
    pub links: Option<u8>,
    pub channels: Channels,
}

const KIB: u64 = 1024;
const MIB: u64 = 1024 * 1024;

const fn band_limits(requests: u32, burst: u32, messages: u32, attachments: u8, bytes: u64, links: Option<u8>, test: bool, dev: bool) -> Limits {
    Limits { requests_per_day: requests, burst, messages_per_day: messages, attachments, attachment_bytes: bytes, links, channels: Channels { test, dev } }
}

/// The eight bands, from trust 0 … 127 to 896 … 1023.
pub const BANDS: [Limits; 8] = [
    band_limits(1, 1, 20, 1, 256 * KIB, Some(3), false, false),
    band_limits(2, 2, 40, 2, 512 * KIB, Some(3), false, false),
    band_limits(4, 3, 80, 4, MIB, None, true, false),
    band_limits(8, 4, 120, 4, MIB, None, true, false),
    band_limits(16, 6, 160, 4, 2 * MIB, None, true, true),
    band_limits(32, 8, 200, 4, 4 * MIB, None, true, true),
    band_limits(64, 12, 300, 4, 4 * MIB, None, true, true),
    band_limits(128, 16, 400, 4, 8 * MIB, None, true, true),
];

/// A device the admin has not approved yet: one held request at a time, a few messages to its registration, nothing
/// attached, the public channel alone.
pub const PENDING: Limits = band_limits(1, 1, 5, 0, 0, Some(3), false, false);

/// What a device may do: by its user's trust once approved, as a pending one before.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// −1023: every call refused, naming the ban.
    Banned,
    /// A negative trust: the user's own requests and their answers are read, nothing new is taken.
    ReadOnly,
    /// Not approved yet.
    Pending(Limits),
    /// Approved, within its band.
    Open(Limits),
}

pub fn access(trust: Trust, approved: bool) -> Access {
    match trust.band() {
        _ if trust.0 == MIN => Access::Banned,
        None => Access::ReadOnly,
        Some(_) if !approved => Access::Pending(PENDING),
        Some(band) => Access::Open(BANDS[band as usize]),
    }
}
