// Where a name on the link goes (800-LNK-0027): below data/link/, or, for `firmware` and the names under it, below
// data/firmware/, which init lets link write in developer mode (800-KRN-0173). Host-tested in tests/link_names_host.rs.

pub const DIR: &str = "data/link";
pub const FIRMWARE: &str = "data/firmware";
/// The name data/firmware/ has on the link.
pub const FIRMWARE_NAME: &str = "firmware";
/// Where a put is written until its SHA-256 matches, in each place; left out of listings.
pub const PART: &str = "data/link/link-part.tmp";
pub const FIRMWARE_PART: &str = "data/firmware/link-part.tmp";
pub const PART_NAME: &str = "link-part.tmp";
pub const PATH_MAX: usize = 320;

/// Whether `name` is data/firmware/ or below it.
pub fn firmware(name: &str) -> bool {
    name == FIRMWARE_NAME || name.len() > FIRMWARE_NAME.len() && name.starts_with(FIRMWARE_NAME) && name.as_bytes()[FIRMWARE_NAME.len()] == b'/'
}

/// The path of `name` in its place: `data/link/<name>`, or `data/firmware` and what follows `firmware`.
pub fn path<'a>(name: &str, out: &'a mut [u8; PATH_MAX]) -> &'a str {
    let parts = if firmware(name) { [FIRMWARE, "", &name[FIRMWARE_NAME.len()..]] } else { [DIR, "/", name] };
    let mut n = 0;
    for part in parts {
        let take = part.len().min(PATH_MAX - n);
        out[n..n + take].copy_from_slice(&part.as_bytes()[..take]);
        n += take;
    }
    core::str::from_utf8(&out[..n]).unwrap_or(PART)
}

/// The part file of a put of `name`, in the same place, so that the finished file is renamed within one client.
pub fn part(name: &str) -> &'static str { if firmware(name) { FIRMWARE_PART } else { PART } }

/// Whether a request may change `name`: never data/firmware/ itself, which init made and link's client is confined to.
pub fn changeable(name: &str) -> bool { name != FIRMWARE_NAME }

/// Whether a move stays in one place: the two are separate clients, and neither reaches the other's files.
pub fn one_place(from: &str, to: &str) -> bool { firmware(from) == firmware(to) && changeable(from) && changeable(to) }

/// A listing's own entries, which FAT keeps in each directory, and the put in progress at a place's top. With the
/// firmware place, a directory data/link/firmware/ is not listed at the top: the name is data/firmware/'s.
pub fn hidden(name: &str, top: bool, firmware_held: bool) -> bool {
    name == "." || name == ".." || top && (name == PART_NAME || firmware_held && name == FIRMWARE_NAME)
}
