//! 213-DRV-0099: what display does at start, by the word after the device's index in its argument (`24 light`), which
//! init passes from the machine's `data/displays.txt` line `output second off|read|light`. No word, or one it does not
//! know, is `off`, and the log says which. Three build switches stay for bring-up only (`Bringup`), off in a release, and
//! a fourth says how far display goes on HDMI-A-2 (213-DRV-0112), its blocks read only in a release; a build with one set
//! says so in its first line. `tests/display_light_host.rs` checks it.

use crate::batches::{Order, BATCHES};
use crate::dcn::{Output, FIRST_LIGHT, STAGES};
use crate::light::{HDMI_A_2, SHORT_WATCH_FRAMES, WATCH_FRAMES};
use core::fmt;

/// The machine's setting for its second output (HDMI-A-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    /// No register mapped, nothing read or written.
    Off,
    /// Step 1 read, the stages described; nothing written (step 2 is left out: it writes the I2C engine).
    Read,
    /// Steps 1 to 4 in the production order, the EDID deciding the mode, then HPD 1 followed and the standing check.
    Light,
}

/// How the setting was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Given<'a> { Word, Missing, Unknown(&'a str) }

/// display's argument: the device's index and the setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Args<'a> { pub device: &'a str, pub setting: Setting, pub given: Given<'a> }

/// The device's index, then one word, its case aside; none, more, or another word is `Off`.
pub fn parse(args: &str) -> Args<'_> {
    let args = args.trim();
    let device = args.split_ascii_whitespace().next().unwrap_or("");
    let word = args[device.len()..].trim();
    let known = [("off", Setting::Off), ("read", Setting::Read), ("light", Setting::Light)];
    match known.iter().find(|(w, _)| w.eq_ignore_ascii_case(word)) {
        Some(&(_, setting)) => Args { device, setting, given: Given::Word },
        None if word.is_empty() => Args { device, setting: Setting::Off, given: Given::Missing },
        None => Args { device, setting: Setting::Off, given: Given::Unknown(word) },
    }
}

/// At most this many bytes of an unknown word go into the log, cut at a character.
pub const WORD_SHOWN: usize = 32;

impl fmt::Display for Args<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let what = match self.setting {
            Setting::Off => "NO REGISTER MAPPED, NOTHING READ OR WRITTEN",
            Setting::Read => "STEP 1 READ, THE STAGES DESCRIBED, NOTHING WRITTEN",
            Setting::Light => "STEPS 1 TO 4, THE EDID DECIDING THE MODE, THEN HPD 1 FOLLOWED AND THE STANDING CHECK",
        };
        let name = match self.setting { Setting::Off => "OFF", Setting::Read => "READ", Setting::Light => "LIGHT" };
        match self.given {
            Given::Word => write!(f, "THE SECOND OUTPUT: {}, THE WORD AFTER DEVICE {}: {}", name, self.device, what),
            Given::Missing => write!(f, "THE SECOND OUTPUT: OFF, NO WORD AFTER DEVICE {}: {}", self.device, what),
            Given::Unknown(word) => {
                let end = (0..=WORD_SHOWN.min(word.len())).rev().find(|&at| word.is_char_boundary(at)).unwrap_or(0);
                write!(f, "THE SECOND OUTPUT: OFF, \"{}{}\" AFTER DEVICE {} IS NOT OFF, READ OR LIGHT: {}", &word[..end], if end < word.len() { "..." } else { "" }, self.device, what)
            }
        }
    }
}

/// The build switches kept for bring-up (213-DRV-0099), all off in a release: the survey's order (213-DRV-0098), the
/// stages undone after they ran, fewer stages than all 16, HDMI-A-2 driven further than its blocks read, and HDMI-A-2
/// taken first when both sockets have a monitor (213-DRV-0112), and a second HDMI run beside the first (213-DRV-0113).
/// They change only what `Light` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bringup { pub survey_order: bool, pub undo_after_stages: bool, pub stages_allowed: u8, pub hdmi_a_2: HdmiA2, pub hdmi_first: HdmiFirst, pub second_hdmi: SecondHdmi }

impl Bringup {
    pub const RELEASE: Bringup = Bringup { survey_order: false, undo_after_stages: false, stages_allowed: STAGES, hdmi_a_2: HdmiA2::Read, hdmi_first: HdmiFirst::HdmiA1,
                                           second_hdmi: SecondHdmi::Off };
    pub fn is_release(&self) -> bool {
        !self.survey_order && !self.undo_after_stages && self.stages_allowed >= STAGES && self.hdmi_a_2 == HdmiA2::Read && self.hdmi_first == HdmiFirst::HdmiA1
            && self.second_hdmi == SecondHdmi::Off
    }
}

impl fmt::Display for Bringup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_release() { return f.write_str("A RELEASE BUILD: NO BRING-UP SWITCH SET"); }
        f.write_str("A BRING-UP BUILD, NOT A RELEASE:")?;
        if self.survey_order { f.write_str(" SURVEY_ORDER")?; }
        if self.undo_after_stages { f.write_str(" UNDO_AFTER_STAGES")?; }
        if self.stages_allowed < STAGES { write!(f, " STAGES_ALLOWED {} OF {}", self.stages_allowed, STAGES)?; }
        if self.hdmi_a_2 != HdmiA2::Read { write!(f, " HDMI_A_2 {}", self.hdmi_a_2.word())?; }
        if self.hdmi_first != HdmiFirst::HdmiA1 { write!(f, " HDMI_FIRST {}", self.hdmi_first.word())?; }
        // Named only: its word, with every switch set, would take the line past 180 bytes; the start's lines name it.
        if self.second_hdmi != SecondHdmi::Off { f.write_str(" SECOND_HDMI")?; }
        f.write_str("; ONLY WHAT LIGHT DOES CHANGES")
    }
}

/// How far display goes on HDMI-A-2, the second HDMI socket (213-DRV-0112): its blocks read only, as a release keeps it
/// until the maintainer has seen them; then its EDID over DDC2 with its stages described; first light's eight stages;
/// all sixteen. Each level does what the ones before it do; the build's STAGES_ALLOWED bounds both outputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HdmiA2 { Read, Edid, FirstLight, Full }

impl HdmiA2 {
    pub fn word(self) -> &'static str { match self { HdmiA2::Read => "READ", HdmiA2::Edid => "EDID", HdmiA2::FirstLight => "FIRST_LIGHT", HdmiA2::Full => "FULL" } }
    /// What it lets display do there, for the log.
    pub fn what(self) -> &'static str {
        match self {
            HdmiA2::Read => "ITS BLOCKS READ ONLY", HdmiA2::Edid => "ITS EDID READ OVER DDC2, ITS STAGES DESCRIBED",
            HdmiA2::FirstLight => "ITS EDID AND FIRST LIGHT'S 8 STAGES", HdmiA2::Full => "ITS EDID AND ALL 16 STAGES",
        }
    }
}

/// Which HDMI socket steps 2 to 4 take when both have a monitor (213-DRV-0112): HDMI-A-1, as a release does, or HDMI-A-2
/// in a bench build, HDMI-A-1 then dark and named as not driven. HDMI-A-2 only where `HdmiA2` lets display take it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HdmiFirst { HdmiA1, HdmiA2 }

impl HdmiFirst {
    pub fn word(self) -> &'static str { match self { HdmiFirst::HdmiA1 => "HDMI-A-1", HdmiFirst::HdmiA2 => "HDMI-A-2" } }
}

/// The second HDMI run (213-DRV-0113), beside the first socket lit, on the other socket's monitor: none, as a release does;
/// the pipe it would take read first and its stages described; first light's eight stages there, left in place or undone
/// after them. On HDMI-A-2 never further than `HdmiA2` lets display go; STAGES_ALLOWED bounds it too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondHdmi { Off, Read, FirstLight, FirstLightUndone }

impl SecondHdmi {
    pub fn word(self) -> &'static str {
        match self { SecondHdmi::Off => "OFF", SecondHdmi::Read => "READ", SecondHdmi::FirstLight => "FIRST_LIGHT", SecondHdmi::FirstLightUndone => "FIRST_LIGHT_UNDONE" }
    }
    pub fn what(self) -> &'static str {
        match self {
            SecondHdmi::Off => "NONE", SecondHdmi::Read => "ITS PIPE READ FIRST, ITS STAGES DESCRIBED",
            SecondHdmi::FirstLight => "FIRST LIGHT'S 8 STAGES, LEFT IN PLACE", SecondHdmi::FirstLightUndone => "FIRST LIGHT'S 8 STAGES, UNDONE AFTER THEM",
        }
    }
}

/// What display's start does, from the setting and the build's bring-up switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub setting: Setting,
    pub order: Order,
    /// Step 1's batches: at 0 no register is mapped or read.
    pub batches: u8,
    /// Step 2, the EDID over DDC1; with it the EDID decides the mode and the signalling (`light::mode_for`).
    pub edid: bool,
    /// Steps 3 and 4's stages: at 0 they are only described.
    pub stages: u8,
    pub undo_after_stages: bool,
    /// HPD 1 followed after the stages (213-DRV-0096).
    pub hotplug: bool,
    /// The standing underflow check after the offer (213-DRV-0103).
    pub standing_check: bool,
    /// Stage 15's watch: the short one only together with the standing check.
    pub watch_frames: u32,
    /// How far display goes on HDMI-A-2 (213-DRV-0112).
    pub hdmi_a_2: HdmiA2,
    /// Which socket is taken when both have a monitor (213-DRV-0112).
    pub hdmi_first: HdmiFirst,
    /// The second HDMI run beside the first (213-DRV-0113).
    pub second_hdmi: SecondHdmi,
}

impl Plan {
    /// Whether anything is written (step 2's I2C engine, or a stage): only then is a writer made.
    pub fn writes(&self) -> bool { self.edid || self.stages > 0 }
    /// Whether steps 2 to 4 may take output `o` when its monitor is the one there (213-DRV-0112): HDMI-A-1 always;
    /// HDMI-A-2 when the plan writes nothing (its stages only described), or from `HdmiA2::Edid` on.
    pub fn takes(&self, o: &Output) -> bool { *o != HDMI_A_2 || !self.writes() || self.hdmi_a_2 >= HdmiA2::Edid }
    /// Whether step 2 reads `o`'s EDID.
    pub fn edid_on(&self, o: &Output) -> bool { self.edid && (*o != HDMI_A_2 || self.hdmi_a_2 >= HdmiA2::Edid) }
    /// How many stages the second HDMI run may run on `o` (213-DRV-0113): first light's at most, as `stages_on` bounds `o`.
    pub fn second_stages(&self, o: &Output) -> u8 {
        match self.second_hdmi { SecondHdmi::Off | SecondHdmi::Read => 0, _ => self.stages_on(o).min(FIRST_LIGHT) }
    }
    /// Whether the second HDMI run's stages are undone after they ran.
    pub fn second_undone(&self) -> bool { self.second_hdmi == SecondHdmi::FirstLightUndone }
    /// How many stages `o` may run.
    pub fn stages_on(&self, o: &Output) -> u8 {
        if *o != HDMI_A_2 { return self.stages; }
        match self.hdmi_a_2 { HdmiA2::Read | HdmiA2::Edid => 0, HdmiA2::FirstLight => self.stages.min(FIRST_LIGHT), HdmiA2::Full => self.stages }
    }
}

/// The plan for `setting`: `Off` and `Read` whatever the build; `Light` as boots 95 to 100 ran it on the Ryzen (11
/// batches, the EDID, 16 stages, HPD 1 followed, the standing check, the watch of 60), less what a bring-up switch takes,
/// and on HDMI-A-2 as far as the build's `HdmiA2` (213-DRV-0112).
pub fn plan(setting: Setting, b: Bringup) -> Plan {
    let order = if b.survey_order { Order::Survey } else { Order::Production };
    let none = Plan { setting, order, batches: 0, edid: false, stages: 0, undo_after_stages: false, hotplug: false, standing_check: false, watch_frames: WATCH_FRAMES,
                      hdmi_a_2: HdmiA2::Read, hdmi_first: HdmiFirst::HdmiA1, second_hdmi: SecondHdmi::Off };
    match setting {
        Setting::Off => none,
        Setting::Read => Plan { batches: BATCHES, ..none },
        Setting::Light => {
            let stages = b.stages_allowed.min(STAGES);
            // What follows the stages needs them in place, not undone; the standing check needs stage 15's picture.
            let follows = stages > 0 && !b.undo_after_stages;
            let standing_check = follows && stages >= 15;
            Plan { batches: BATCHES, edid: true, stages, undo_after_stages: b.undo_after_stages && stages > 0, hotplug: follows, standing_check,
                   watch_frames: if standing_check { SHORT_WATCH_FRAMES } else { WATCH_FRAMES }, hdmi_a_2: b.hdmi_a_2, hdmi_first: b.hdmi_first, second_hdmi: b.second_hdmi, ..none }
        }
    }
}
