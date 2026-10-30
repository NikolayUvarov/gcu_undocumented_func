//! `update`'s command line and text (351-APP-0029): the updater's view, the channel it holds (`update.wit` 1.1
//! `metadata`, for 351-UPD-0025) and its answers; the calls are `updatectl.rs`'. Host-tested in tests/shell_host.rs.
use core::fmt::Write;

/// What `update` was asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked { Status, Check, Fetch, Apply(u8), Rollback }

pub const USAGE: &str = "USAGE: UPDATE [STATUS | CHECK | FETCH | APPLY [TRIES, 1 TO 9] | ROLLBACK]";
/// Tries of a trial boot when `apply` names none, as update.txt's example (docs/update/updater.md).
pub const TRIES: u8 = 3;
/// `update.wit` error codes the shell answers itself before it asks the operator.
pub const NO_SLOT: usize = 1;
pub const TRIAL: usize = 2;

/// `update` alone is `update status`.
pub fn parse(args: &str) -> Result<Asked, &'static str> {
    let mut words = args.split_whitespace();
    let asked = match words.next().map(|w| w.to_ascii_lowercase()).as_deref() {
        None | Some("status") => Asked::Status,
        Some("check") => Asked::Check,
        Some("fetch") => Asked::Fetch,
        Some("rollback") => Asked::Rollback,
        Some("apply") => match words.next().map(str::parse::<u8>) {
            None => Asked::Apply(TRIES),
            Some(Ok(tries @ 1..=9)) => Asked::Apply(tries),
            Some(_) => return Err(USAGE),
        },
        Some(_) => return Err(USAGE),
    };
    if words.next().is_some() { return Err(USAGE); }
    Ok(asked)
}

/// An `update.wit` error, by its code.
pub fn error(code: usize) -> &'static str {
    match code {
        0 => "UPDATE.TXT NAMES NO SOURCE",
        NO_SLOT => "THE SYSTEM DID NOT BOOT FROM A SLOT",
        TRIAL => "THE RUNNING SLOT IS ON TRIAL: THE OTHER ONE IS ITS FALLBACK AND STAYS AS IT IS",
        3 => "THE SOURCE DID NOT ANSWER, OR A TRANSFER FAILED AFTER ITS TRIES",
        4 => "THE SOURCE HAS NO SUCH FILE, OR THE CHANNEL NO MANIFEST FOR THIS ARCHITECTURE",
        5 => "THE PARSER REFUSED THE METADATA",
        6 => "A CHANNEL OR MANIFEST NOT SIGNED BY ITS KEY",
        7 => "THE CHANNEL HAS EXPIRED, OR THE CLOCK CANNOT BE READ",
        8 => "THE CHANNEL NAMES THE RUNNING VERSION OR AN OLDER ONE",
        9 => "A FILE NOT AS ITS MANIFEST LISTS IT",
        10 => "THE SLOT OR A BOOT RECORD COULD NOT BE WRITTEN",
        11 => "NO VERIFIED NEWER RELEASE IN THE OTHER SLOT",
        12 => "NO SLOT TO ROLL BACK TO, OR ITS VERSION IS NOT KNOWN OR BELOW THE MINIMUM",
        13 => "INIT REFUSED THE RESTART",
        14 => "THE PARSER SERVICE DID NOT ANSWER",
        _ => "THE UPDATER ANSWERED AN ERROR",
    }
}

/// The updater's `state`: the slot as a letter (`b'A'`, `b'B'`, 0 for none), versions with 0 for not known or none.
pub struct View<'a> { pub slot: u8, pub trial: bool, pub running: u64, pub recorded: u8, pub confirmed: bool, pub staged: u64, pub failure: &'a str }

/// The slot the system did not boot from.
pub fn other(slot: u8) -> char { if slot == b'A' { 'B' } else { 'A' } }

struct Version(u64);

impl core::fmt::Display for Version {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result { if self.0 == 0 { f.write_str("NOT KNOWN") } else { write!(f, "{}", self.0) } }
}

pub fn state(out: &mut impl Write, view: &View) {
    // A trial boot is over once the newer record names the running slot confirmed, as the updater stages again then.
    let trial = view.trial && !(view.recorded == view.slot && view.confirmed);
    let _ = match view.slot {
        0 => writeln!(out, "UPDATE: RUNNING FROM THE VOLUME'S ROOT, NOT A SLOT, VERSION {}", Version(view.running)),
        slot => writeln!(out, "UPDATE: RUNNING SLOT {}{}, VERSION {}", slot as char, if trial { " ON TRIAL" } else { "" }, Version(view.running)),
    };
    let _ = match view.recorded {
        0 => writeln!(out, "UPDATE: NO BOOT RECORD NAMES A SLOT"),
        slot => writeln!(out, "UPDATE: THE NEWER BOOT RECORD NAMES SLOT {}, {}", slot as char, if view.confirmed { "CONFIRMED" } else { "NOT CONFIRMED" }),
    };
    let _ = match view.staged {
        0 => writeln!(out, "UPDATE: NOTHING STAGED IN THE OTHER SLOT"),
        version => writeln!(out, "UPDATE: STAGED IN SLOT {}: VERSION {}", other(view.slot), version),
    };
    if !view.failure.is_empty() { let _ = writeln!(out, "UPDATE: THE LAST FAILURE: {}", view.failure); }
}

/// `metadata`: the channel the updater holds, read from the source or kept in the running slot.
pub fn channel(out: &mut impl Write, name: &str, version: u64, minimum: u64, expires: &str, expired: bool, checked: bool) {
    if name.is_empty() { let _ = writeln!(out, "UPDATE: THE UPDATER HOLDS NO CHANNEL"); return; }
    let whence = if checked { "READ FROM THE SOURCE" } else { "KEPT IN THE RUNNING SLOT, NOT READ FROM THE SOURCE SINCE THE UPDATER STARTED" };
    let _ = writeln!(out, "UPDATE: CHANNEL {}: VERSION {}, MINIMUM {}, EXPIRES {}, {}", name, Version(version), Version(minimum), expires, whence);
    if expired { let _ = writeln!(out, "UPDATE: THE CHANNEL HAS EXPIRED, OR THE CLOCK CANNOT BE READ: FETCH AND APPLY WAIT FOR A CURRENT ONE, ROLLBACK STAYS POSSIBLE"); }
}

/// `check`'s offer.
pub fn offer(out: &mut impl Write, name: &str, version: u64, minimum: u64, expires: &str, running: u64) {
    let _ = writeln!(out, "UPDATE: CHANNEL {} OFFERS VERSION {} (MINIMUM {}, EXPIRES {}); RUNNING VERSION {}", name, version, Version(minimum), expires, Version(running));
}

/// The question before `apply`, which restarts the machine. `staged` is 0 after a restart, while the other slot may still
/// hold a release staged before it: the updater checks that slot on the disk.
pub fn apply_question(out: &mut impl Write, view: &View, tries: u8) {
    let _ = match view.staged {
        0 => write!(out, "APPLY THE RELEASE IN SLOT {} (RUNNING {})?", other(view.slot), Version(view.running)),
        version => write!(out, "APPLY VERSION {} (RUNNING {})?", version, Version(view.running)),
    };
    let _ = write!(out, " THE MACHINE RESTARTS INTO SLOT {} ON TRIAL WITH {} TR{}, FALLING BACK TO SLOT {}.", other(view.slot), tries, if tries == 1 { "Y" } else { "IES" }, view.slot as char);
}

/// The question before `rollback`, which restarts the machine.
pub fn rollback_question(out: &mut impl Write, view: &View) {
    let _ = write!(out, "ROLL BACK TO SLOT {}? IF ITS VERSION IS NOT BELOW THE MINIMUM, THE MACHINE RESTARTS INTO IT ON TRIAL, FALLING BACK TO SLOT {}.",
                   other(view.slot), view.slot as char);
}

/// Why the shell answers `apply` or `rollback` itself, without a question: the updater refuses both without a slot or on
/// trial. Whether the other slot holds a newer release is the updater's to check.
pub fn refused(view: &View) -> Option<usize> {
    if view.slot == 0 { Some(NO_SLOT) } else if view.trial { Some(TRIAL) } else { None }
}
