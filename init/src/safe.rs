// Which services a start after an unfinished run leaves out (000-KRN-0217). Each service of the safe start's list is
// marked while its start is under SETTLE_MS old; after a run that did not end as asked only those still marked are
// suspected. They are left out at once with evidence of a freeze (the watchdog restarted the run), else only after two
// such runs in a row. Pure: the kept bytes in, the decision out; init keeps the bytes in a firmware variable.

/// The marks' variable (vendor GUID RUN_MARK_GUID) and its format's version, the first byte.
pub const NAME: &str = "MindCoreStarts";
pub const VERSION: u8 = 1;
/// How long a start stays marked: a window, not a readiness the services report (they report none).
pub const SETTLE_MS: u64 = 30_000;
/// After a safe start, how long past READY without a service's fault before the left-out services start again.
pub const RETURN_MS: u64 = 300_000;

/// The kept state, a bit for each service of the safe start's list in its order: those whose start was marked when the
/// run ended, and those suspected once already, by a run cut short without evidence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Marks { pub starting: u8, pub suspected: u8 }

/// Why the marks read as they did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Read { Kept, None, Version(u8), Damaged }

impl Marks {
    /// The variable's bytes; none, another version or another length is no marks, with the reason.
    pub fn from_bytes(bytes: Option<&[u8]>) -> (Marks, Read) {
        match bytes {
            None => (Marks::default(), Read::None),
            Some([VERSION, starting, suspected]) => (Marks { starting: *starting, suspected: *suspected }, Read::Kept),
            Some([version, ..]) if *version != VERSION => (Marks::default(), Read::Version(*version)),
            Some(_) => (Marks::default(), Read::Damaged),
        }
    }
    pub fn bytes(&self) -> [u8; 3] { [VERSION, self.starting, self.suspected] }
}

/// What the last run's end means for this boot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// It ended as asked (no run's mark): nothing is left out.
    Ended,
    /// It ended early with no service of the list starting: a normal start.
    Nothing,
    /// It ended early while these were starting, with no evidence of a freeze: a normal start, and they are suspected.
    Once,
    /// The same, a second run in a row: left out.
    Twice,
    /// The watchdog restarted it while these were starting: left out at once.
    Evidence,
}

/// This boot's decision: the services left out and the marks to keep for the next one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision { pub left_out: u8, pub named: u8, pub kept: Marks, pub why: Why }

/// The decision after the last run: `unfinished` when its mark was found, `evidence` when the watchdog restarted it.
pub fn decide(unfinished: bool, evidence: bool, last: Marks) -> Decision {
    let none = Marks::default();
    if !unfinished { return Decision { left_out: 0, named: 0, kept: none, why: Why::Ended }; }
    let suspects = last.starting;
    if suspects == 0 { return Decision { left_out: 0, named: 0, kept: none, why: Why::Nothing }; }
    if evidence { return Decision { left_out: suspects, named: suspects, kept: none, why: Why::Evidence }; }
    match suspects & last.suspected {
        0 => Decision { left_out: 0, named: suspects, kept: Marks { starting: 0, suspected: suspects }, why: Why::Once },
        twice => Decision { left_out: twice, named: twice, kept: none, why: Why::Twice },
    }
}

/// The bit of the `index`-th service of the list.
pub fn bit(index: usize) -> u8 { if index < 8 { 1 << index } else { 0 } }
