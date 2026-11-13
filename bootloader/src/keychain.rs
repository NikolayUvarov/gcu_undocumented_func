// A volume's key file under the root built in (351-UPD-0029, docs/update/slots.md; MC-9.5, 9.6). Free of UEFI so the
// host tests (tests/boot_keys_host.rs) run the same check the bootloader makes, and the updater checks a slot it writes.
//
// A directory holds `KEYS`, the key file whose boot role signs its manifest, and `KEYS.1`, `KEYS.2`, … the key files
// before it at which the root changed, in order. Each is signed by the root trusted before it, the built-in one first,
// and by its own: the bootloader holds no history, only the highest version the boot records name.
use super::release::{self, Keys, Role};

/// Files of root changes a directory holds, at most.
pub const ROTATIONS_MAX: usize = 8;
/// Their names, in order.
pub const ROTATIONS: [&str; ROTATIONS_MAX] = ["KEYS.1", "KEYS.2", "KEYS.3", "KEYS.4", "KEYS.5", "KEYS.6", "KEYS.7", "KEYS.8"];

/// Why a directory's key files are refused, with the file it was found in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The file could not be read: the reader's reason.
    Read(&'static str, &'static str),
    /// Not a key file of the one encoding.
    Malformed(&'static str),
    /// Its version is not above the one before it.
    Order(&'static str, u64, u64),
    /// Fewer than the threshold of the root trusted before it signed it; that root's version (0: the built-in one).
    Root(&'static str, u64),
    /// Fewer than its own root's threshold signed it.
    Own(&'static str),
    /// `KEYS`' version is below the one the boot records name.
    Below(u64, u64),
}

impl Refusal {
    /// The reason, as the bootloader stops with it.
    pub fn reason(&self) -> &'static str {
        match self {
            Refusal::Read(_, why) => why,
            Refusal::Malformed(_) => "not a key file (MC-9.6)",
            Refusal::Order(..) => "the versions of its key files do not rise (MC-9.6)",
            Refusal::Root(..) => "fewer than the threshold of the root keys signed it (MC-9.6)",
            Refusal::Own(_) => "fewer than the threshold of its own root keys signed it (MC-9.6)",
            Refusal::Below(..) => "below the key file version the boot records name: it may name revoked keys (MC-9.6)",
        }
    }
}

impl core::fmt::Display for Refusal {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match *self {
            Refusal::Read(name, why) => write!(f, "{}: {}", name, why),
            Refusal::Malformed(name) => write!(f, "{}: NOT A KEY FILE", name),
            Refusal::Order(name, version, before) => write!(f, "{}: VERSION {} NOT ABOVE VERSION {} BEFORE IT", name, version, before),
            Refusal::Root(name, 0) => write!(f, "{}: FEWER THAN THE THRESHOLD OF THE ROOT KEYS BUILT IN SIGNED IT", name),
            Refusal::Root(name, before) => write!(f, "{}: FEWER THAN THE THRESHOLD OF THE ROOT KEYS OF VERSION {} SIGNED IT", name, before),
            Refusal::Own(name) => write!(f, "{}: FEWER THAN THE THRESHOLD OF ITS OWN ROOT KEYS SIGNED IT", name),
            Refusal::Below(version, held) => write!(f, "KEYS: VERSION {} BELOW VERSION {}, WHICH THE BOOT RECORDS NAME", version, held),
        }
    }
}

/// What a directory's key files came to: the key file, and how many root changes led to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checked { pub keys: Keys, pub rotations: usize }

/// The root set built in, as a role: its keys in ascending order, as a key file's.
pub fn built_in(keys: &[[u8; 32]], threshold: u8) -> Role {
    let mut role = Role { threshold, ..Role::EMPTY };
    for (to, from) in role.keys.iter_mut().zip(keys) { *to = *from; }
    role.count = keys.len().min(role.keys.len());
    role.keys[..role.count].sort_unstable();
    role
}

/// The directory's key files checked under `root` (the set built in): `KEYS.1` on while they are there, then `KEYS`,
/// whose version must not be below `held`. `read(name, buffer)` gives a file's length in `buffer`, or why not; `parse`
/// reads a key file (`release::parse_keys`, or the parser service's answer); `verify` is the Ed25519 check.
pub fn check(root: Role, held: u64, mut read: impl FnMut(&str, &mut [u8]) -> Result<usize, &'static str>,
             parse: impl Fn(&[u8]) -> Option<(Keys, release::Signed)>, verify: impl Fn(&[u8; 32], &[u8], &[u8; 64]) -> bool) -> Result<Checked, Refusal> {
    let mut buffer = [0u8; release::KEYS_MAX];
    let (mut trusted, mut version, mut rotations) = (root, 0u64, 0);
    let take = |name: &'static str, file: &[u8], trusted: &mut Role, version: &mut u64| -> Result<Keys, Refusal> {
        let (keys, signed) = parse(file).ok_or(Refusal::Malformed(name))?;
        if keys.version <= *version { return Err(Refusal::Order(name, keys.version, *version)); }
        let body = &file[..signed.body];
        if !release::signed_by(trusted.keys(), trusted.threshold, body, &signed, &verify) { return Err(Refusal::Root(name, *version)); }
        let own = keys.root();
        if !release::signed_by(own.keys(), own.threshold, body, &signed, &verify) { return Err(Refusal::Own(name)); }
        (*trusted, *version) = (*own, keys.version);
        Ok(keys)
    };
    for name in ROTATIONS {
        let length = match read(name, &mut buffer) { Ok(length) => length, Err("file not found") => break, Err(why) => return Err(Refusal::Read(name, why)) };
        take(name, &buffer[..length.min(release::KEYS_MAX)], &mut trusted, &mut version)?;
        rotations += 1;
    }
    let length = read("KEYS", &mut buffer).map_err(|why| Refusal::Read("KEYS", why))?;
    let keys = take("KEYS", &buffer[..length.min(release::KEYS_MAX)], &mut trusted, &mut version)?;
    if keys.version < held { return Err(Refusal::Below(keys.version, held)); }
    Ok(Checked { keys, rotations })
}
