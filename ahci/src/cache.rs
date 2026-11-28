// What a flush sends an ATA drive, from its IDENTIFY words (ACS-3, 7.12.7), for `ahci` and `ata` (307-DRV-0055): no
// command when the drive says it has no volatile write cache (word 82 bit 5) or that it is off (word 85 bit 5), else
// FLUSH CACHE EXT where it has 48-bit addressing and that command (word 83 bits 10 and 13), FLUSH CACHE otherwise.
// Words 82 and 83 count only where word 83 says they are valid (bits 15:14 01), words 85 to 87 where word 87 does; a
// drive that says nothing valid gets the command. Built on the host for tests (tests/flush_host.rs).

/// FLUSH CACHE and FLUSH CACHE EXT.
pub const FLUSH_CACHE: u8 = 0xE7;
pub const FLUSH_CACHE_EXT: u8 = 0xEA;

/// What a flush does with the drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flush {
    /// The drive has no volatile write cache: answered done, no command sent.
    Absent,
    /// Its write cache is off, so a write is on the medium when it ends: answered done, no command sent.
    Off,
    /// This command empties its cache.
    Send(u8),
}

/// The flush for a drive whose IDENTIFY word `i` is `word(i)`.
pub fn flush(word: impl Fn(usize) -> u16) -> Flush {
    let valid = |w: u16| w & 0xC000 == 0x4000;
    let (sets, enabled) = (valid(word(83)), valid(word(87)));
    if sets && word(82) & 1 << 5 == 0 { return Flush::Absent; }
    if enabled && word(85) & 1 << 5 == 0 { return Flush::Off; }
    Flush::Send(if sets && word(83) & (1 << 10 | 1 << 13) == 1 << 10 | 1 << 13 { FLUSH_CACHE_EXT } else { FLUSH_CACHE })
}

/// A command's name for the log.
pub fn name(command: u8) -> &'static str { if command == FLUSH_CACHE_EXT { "FLUSH CACHE EXT" } else { "FLUSH CACHE" } }

/// Why no command is sent, for the log.
pub fn why(flush: Flush) -> &'static str {
    match flush { Flush::Absent => "IDENTIFY WORD 82 BIT 5 CLEAR: NONE", Flush::Off => "IDENTIFY WORD 85 BIT 5 CLEAR: IT IS OFF", Flush::Send(_) => "" }
}

/// What a flush's log has said, each kind once: done, no cache, refused, not answered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Said(u8);

impl Said {
    pub const DONE: u8 = 1;
    pub const NO_CACHE: u8 = 2;
    pub const REFUSED: u8 = 4;
    pub const UNANSWERED: u8 = 8;
    pub const fn new() -> Self { Said(0) }
    /// True the first time `kind` happens, when the log says it.
    pub fn first(&mut self, kind: u8) -> bool { let first = self.0 & kind == 0; self.0 |= kind; first }
}
