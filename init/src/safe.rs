// Which services a start after an unfinished run leaves out (000-KRN-0217, 801 S5). The run's mark holds the slot whose
// run set it and a bit for each service of the safe start's list whose start had not settled. Only those are suspected,
// only in a run of the same slot, and only at once with the watchdog's evidence of a freeze, else after two such runs in
// a row. Pure: the mark's bytes in, the decision out; init keeps the bytes in the firmware variable RUN_MARK_NAME.

/// The mark's format, its first byte; version 1 was the one byte `1`, which said nothing of what was starting.
pub const VERSION: u8 = 2;
/// A start settles once it and the boot (READY) have run this long: services report no readiness of their own.
pub const SETTLE_MS: usize = 30_000;
/// After a safe start, how long past READY the left-out services start again, one at a time, each once the one
/// before settled: only a deliberate switch-off keeps the network down (801 S5).
pub const RETURN_MS: usize = 120_000;

/// The run's mark: the slot of the run that set it, the list's services whose start had not settled, and those
/// suspected already by an earlier run cut short. A bit for each service of the list, in its order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mark { pub slot: u8, pub starting: u8, pub suspected: u8 }

/// How the last run's mark read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Read { Kept(Mark), Version(u8), Damaged }

impl Mark {
    pub fn read(bytes: &[u8]) -> Read {
        match bytes {
            [VERSION, slot, starting, suspected] => Read::Kept(Mark { slot: *slot, starting: *starting, suspected: *suspected }),
            [version, ..] if *version != VERSION => Read::Version(*version),
            _ => Read::Damaged,
        }
    }
    pub fn bytes(&self) -> [u8; 4] { [VERSION, self.slot, self.starting, self.suspected] }
}

/// What the last run's end means for this boot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// No mark: it ended as asked.
    Ended,
    /// A mark of another version, or damaged: what was starting is not known, and nothing is suspected.
    Unknown,
    /// The run of another slot (the slot named): what it was starting is not this slot's to suspect (801 S3).
    OtherSlot(u8),
    /// No service of the list was starting.
    Nothing,
    /// These were starting, with no evidence of a freeze: a normal start, and they are suspected.
    Once,
    /// Some were starting the run before too: those are left out.
    Twice,
    /// The watchdog restarted the run: what was starting is left out at once.
    Evidence,
}

/// This boot's decision: the services left out, those named in its line, those suspected for the next boot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision { pub left_out: u8, pub named: u8, pub suspected: u8, pub why: Why }

/// The decision after the last run: `last` none when it left no mark, `slot` this boot's, `evidence` when the
/// watchdog restarted it.
pub fn decide(last: Option<Read>, slot: u8, evidence: bool) -> Decision {
    let plain = |why| Decision { left_out: 0, named: 0, suspected: 0, why };
    let mark = match last {
        None => return plain(Why::Ended),
        Some(Read::Version(_) | Read::Damaged) => return plain(Why::Unknown),
        Some(Read::Kept(mark)) => mark,
    };
    if mark.slot != slot { return plain(Why::OtherSlot(mark.slot)); }
    let suspects = mark.starting;
    if suspects == 0 { return plain(Why::Nothing); }
    if evidence { return Decision { left_out: suspects, named: suspects, suspected: suspects, why: Why::Evidence }; }
    match suspects & mark.suspected {
        0 => Decision { left_out: 0, named: suspects, suspected: suspects, why: Why::Once },
        twice => Decision { left_out: twice, named: twice, suspected: suspects, why: Why::Twice },
    }
}

/// The bit of the `index`-th service of the list.
pub fn bit(index: usize) -> u8 { if index < 8 { 1 << index } else { 0 } }

/// Whether a start made at `started` has settled at `now`, READY having come at `ready` (none: not yet).
pub fn settled(started: usize, ready: Option<usize>, now: usize) -> bool {
    ready.is_some_and(|ready| now >= started.max(ready) + SETTLE_MS)
}

/// The next left-out service to start again, by its index in the list, the radios first (`first`, a mask of the
/// list's radios): none before RETURN_MS past READY, or while another start has not settled.
pub fn next_return(waiting: u8, first: u8, unsettled: bool, ready: Option<usize>, now: usize) -> Option<usize> {
    if waiting == 0 || unsettled || !ready.is_some_and(|ready| now >= ready + RETURN_MS) { return None; }
    let pick = if waiting & first != 0 { waiting & first } else { waiting };
    Some(pick.trailing_zeros() as usize)
}
