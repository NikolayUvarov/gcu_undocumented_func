// The device key sealed under the owner's passphrase where no TPM seals it (idl/keystore.wit 1.6, 308-NET-0055):
// `system/keystore/device.locked`, the file of the passlock crate, which every boot waits on once the owner set it.
// Argon2id's memory is pages of this service's own, wiped by passlock and freed after each derivation; while one runs,
// the service answers nothing else. From the third wrong passphrase in a row, a try before the waiting time (1 s,
// doubling up to 60 s, passlock::Tries) is refused at once with `budget`, nothing derived, so no answer waits on a
// sleep. No passphrase reaches the log; the caller wipes its copies.
use crate::stored;
use mind::fs::{self, File};
use mind::idl::keystore::Error;
use mind::mem::Pages;
use mind::network::BADGE_KEY_PASSPHRASE;
use passlock::{Block, Params, Refused, Tries, LEN, PASSPHRASE_MAX, PASSPHRASE_MIN};

/// A changed file written whole before it replaces the kept one.
pub const NEW: &str = "system/keystore/device.locked.new";

/// What the boot finds of the sealed file.
pub enum Found { File([u8; LEN]), Missing, Damaged, Version(u8), Unreadable(fs::Error) }

fn read(path: &str) -> Found {
    let file = match File::open(path) { Ok(file) => file, Err(fs::Error::NotFound) => return Found::Missing, Err(error) => return Found::Unreadable(error) };
    let mut data = [0u8; LEN + 1];
    let n = match file.read_at(0, &mut data) { Ok(n) => n, Err(error) => return Found::Unreadable(error) };
    match passlock::header(&data[..n]) {
        Ok(_) => { let mut kept = [0u8; LEN]; kept.copy_from_slice(&data[..LEN]); Found::File(kept) }
        Err(Refused::Version(version)) => Found::Version(version),
        Err(_) => Found::Damaged,
    }
}

/// The sealed file: `device.locked`, or the new one a change cut short after it removed the old one.
pub fn find() -> Found {
    match read(passlock::FILE) {
        Found::Missing => match read(NEW) {
            // Flushed whole before the old one was removed: the change is finished here.
            Found::File(file) => {
                let renamed = fs::rename(NEW, passlock::FILE).is_ok();
                mind::println!("[KEYSTORE] A CHANGE OF THE PASSPHRASE WAS CUT SHORT: {} {}", NEW, if renamed { "PUT IN PLACE" } else { "USED, NOT RENAMED" });
                Found::File(file)
            }
            // Cut while it was written, before anything was removed: it replaced nothing.
            _ => Found::Missing,
        },
        found => found,
    }
}

/// Whether a sealed file is kept, read or not.
pub fn present() -> bool { !matches!(read(passlock::FILE), Found::Missing) || !matches!(read(NEW), Found::Missing) }

// Argon2id's memory: zeroed pages of this service's own, freed when dropped, after passlock wiped them.
struct Memory(Pages);

impl Memory {
    fn new(params: Params) -> Result<Self, Error> {
        let blocks = params.blocks().ok_or(Error::Invalid)?;
        Pages::new(blocks * Block::SIZE).map(Memory).ok_or_else(|| {
            mind::println!("[KEYSTORE] NO MEMORY FOR ARGON2ID: {} KIB", params.memory_kib);
            Error::NoKey
        })
    }

    fn blocks(&mut self) -> &mut [Block] {
        // Page-aligned zeroed pages, whole blocks: valid as Argon2id's blocks of 128 words each.
        unsafe { core::slice::from_raw_parts_mut(self.0.address() as *mut Block, self.0.len() / Block::SIZE) }
    }
}

// Argon2id's memory for `params`, given to `f`, and the derivation's time in milliseconds.
fn derive<T>(params: Params, f: impl FnOnce(&mut [Block]) -> T) -> Result<(T, usize), Error> {
    let mut memory = Memory::new(params)?;
    let started = mind::time::uptime_ms();
    let result = f(memory.blocks());
    Ok((result, mind::time::uptime_ms().saturating_sub(started)))
}

/// A call only the passphrase's badge may make: `denied` to any other, in the log.
pub fn client(badge: u16, pid: u64, what: &str) -> Result<(), Error> {
    if badge != BADGE_KEY_PASSPHRASE { mind::println!("[KEYSTORE] REFUSED {} FOR PID {}: NOT THE PASSPHRASE'S CLIENT", what, pid); return Err(Error::Denied); }
    Ok(())
}

/// The sealed file kept, and the wrong passphrases in a row.
pub struct Lock { pub file: Option<[u8; LEN]>, tries: Tries }

fn now() -> u64 { mind::time::uptime_ms() as u64 }

impl Lock {
    pub fn new() -> Self { Self { file: None, tries: Tries::default() } }

    // `budget` for a try before the waiting time.
    fn early(&self, pid: u64, what: &str) -> Result<(), Error> {
        let Some(left) = self.tries.early(now()) else { return Ok(()) };
        mind::println!("[KEYSTORE] REFUSED {} FOR PID {}: {} MS BEFORE THE NEXT TRY ({} WRONG IN A ROW)", what, pid, left, self.tries.in_a_row());
        Err(Error::Budget)
    }

    // One more wrong passphrase in a row: from the third, the next try waits.
    fn wrong(&mut self, pid: u64, what: &str, ms: usize) -> Error {
        let wait = self.tries.wrong(now());
        let row = self.tries.in_a_row();
        match wait {
            0 => mind::println!("[KEYSTORE] REFUSED {} FOR PID {}: A WRONG PASSPHRASE ({} IN A ROW, ARGON2ID {} MS)", what, pid, row, ms),
            _ => mind::println!("[KEYSTORE] REFUSED {} FOR PID {}: A WRONG PASSPHRASE ({} IN A ROW, ARGON2ID {} MS; THE NEXT TRY AFTER {} S)", what, pid, row, ms, wait / 1000),
        }
        Error::Denied
    }

    // The seed `file` seals, opened by `passphrase`: `denied` for a wrong one, counted.
    fn open(&mut self, file: &[u8; LEN], pid: u64, what: &str, passphrase: &[u8]) -> Result<([u8; 32], usize), Error> {
        self.early(pid, what)?;
        let params = passlock::header(file).map_err(|_| Error::NoKey)?.params;
        let (opened, ms) = derive(params, |memory| passlock::open(file, passphrase, memory))?;
        match opened {
            Ok(seed) => { self.tries.opened(); Ok((seed, ms)) }
            Err(Refused::Wrong) => Err(self.wrong(pid, what, ms)),
            // Never sealed under: nothing derived, nothing counted.
            Err(Refused::Passphrase) => { mind::println!("[KEYSTORE] REFUSED {} FOR PID {}: A PASSPHRASE IS {} TO {} BYTES", what, pid, PASSPHRASE_MIN, PASSPHRASE_MAX); Err(Error::Denied) }
            Err(refused) => { mind::println!("[KEYSTORE] REFUSED {} FOR PID {}: {:?}", what, pid, refused); Err(Error::NoKey) }
        }
    }

    /// The seed of the waiting file, opened by `passphrase`; the caller wipes it.
    pub fn unlock(&mut self, pid: u64, passphrase: &[u8]) -> Result<[u8; 32], Error> {
        let Some(file) = self.file else { return Err(Error::Invalid) };
        let (seed, ms) = self.open(&file, pid, "UNLOCK", passphrase)?;
        mind::println!("[KEYSTORE] DEVICE KEY FROM {} (SEALED UNDER THE PASSPHRASE), OPENED FOR PID {} (ARGON2ID {} MS)", passlock::FILE, pid, ms);
        Ok(seed)
    }

    /// Seals the device key's `seed` under `new` in place of `old` (empty when none is set), then removes its
    /// unencrypted file.
    pub fn set(&mut self, pid: u64, seed: &[u8; 32], old: &[u8], new: &[u8]) -> Result<(), Error> {
        const WHAT: &str = "PASSPHRASE-SET";
        if !(PASSPHRASE_MIN..=PASSPHRASE_MAX).contains(&new.len()) {
            mind::println!("[KEYSTORE] REFUSED {} FOR PID {}: THE NEW ONE IS NOT {} TO {} BYTES", WHAT, pid, PASSPHRASE_MIN, PASSPHRASE_MAX);
            return Err(Error::Invalid);
        }
        let changed = match (self.file, old.is_empty()) {
            (None, true) => false,
            (None, false) => { mind::println!("[KEYSTORE] REFUSED {} FOR PID {}: NONE IS SET, SO THE OLD ONE IS EMPTY", WHAT, pid); return Err(Error::Denied); }
            (Some(_), true) => { mind::println!("[KEYSTORE] REFUSED {} FOR PID {}: ONE IS SET, AND THE OLD ONE IS EMPTY", WHAT, pid); return Err(Error::Denied); }
            (Some(file), false) => { let (mut opened, _) = self.open(&file, pid, WHAT, old)?; stored::wipe(&mut opened); true }
        };
        let (mut salt, mut nonce) = ([0u8; passlock::SALT], [0u8; passlock::NONCE]);
        if !mind::random::fill(&mut salt) || !mind::random::fill(&mut nonce) { mind::println!("[KEYSTORE] REFUSED {} FOR PID {}: NO {}", WHAT, pid, mind::random::SOURCE); return Err(Error::NoKey); }
        let params = Params::DEFAULT;
        let (sealed, ms) = derive(params, |memory| passlock::seal(new, &salt, &nonce, params, seed, memory))?;
        let sealed = sealed.map_err(|refused| { mind::println!("[KEYSTORE] REFUSED {} FOR PID {}: {:?}", WHAT, pid, refused); Error::NoKey })?;
        let unfinished = write(&sealed).map_err(|why| { mind::println!("[KEYSTORE] REFUSED {} FOR PID {}: {} {}; THE PASSPHRASE STAYS AS IT WAS", WHAT, pid, NEW, why); Error::NoKey })?;
        self.file = Some(sealed);
        mind::println!("[KEYSTORE] PASSPHRASE {} FOR PID {}: THE DEVICE KEY SEALED UNDER IT IN {} (ARGON2ID {} KIB, {} PASSES, {} LANE{}; {} MS)",
                       if changed { "CHANGED" } else { "SET" }, pid, passlock::FILE, params.memory_kib, params.passes, params.lanes, if params.lanes == 1 { "" } else { "S" }, ms);
        if let Some(why) = unfinished { mind::println!("[KEYSTORE] {}: {}", NEW, why); }
        match stored::remove_plain() {
            Ok(()) => mind::println!("[KEYSTORE] THE UNENCRYPTED COPY {} REMOVED", stored::FILE),
            Err(fs::Error::NotFound) => {}
            Err(error) => mind::println!("[KEYSTORE] THE UNENCRYPTED COPY {} NOT REMOVED: {:?} (REMOVED AT THE NEXT UNLOCK)", stored::FILE, error),
        }
        Ok(())
    }
}

// The new file first, flushed, then in place of the kept one, so that a cut leaves one of them whole (`find`). Err:
// nothing changed; Ok(Some): the old one is removed and the new one holds, but not all of the rest was done.
fn write(file: &[u8; LEN]) -> Result<Option<&'static str>, &'static str> {
    fs::mkdir(stored::DIR).map_err(|_| "NO DIRECTORY")?;
    File::create(NEW).and_then(|mut new| { new.write_at(0, file)?; new.flush() }).map_err(|_| "NOT WRITTEN")?;
    match fs::remove(passlock::FILE) { Ok(()) | Err(fs::Error::NotFound) => {} Err(_) => return Err("WRITTEN, THE OLD FILE NOT REMOVED") }
    let renamed = fs::rename(NEW, passlock::FILE).is_ok();
    // The removal and the rename reach the disk with the flush.
    Ok(match (renamed, stored::flush()) {
        (true, true) => None,
        (false, true) => Some("NOT RENAMED, THE NEW PASSPHRASE HOLDS FROM IT AT THE NEXT BOOT"),
        (_, false) => Some("NOT FLUSHED: THE OLD PASSPHRASE MAY HOLD AT THE NEXT BOOT"),
    })
}
