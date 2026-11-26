//! 213-DRV-0099: what display does at start, by the word after the device's index in its argument (`24 light`), which
//! init passes from the machine's `data/displays.txt` line `output second off|read|light`. No word, or one it does not
//! know, is `off`, and the log says which. Three build switches stay for bring-up only (`Bringup`), off in a release; a
//! build with one set says so in its first line. `tests/display_light_host.rs` checks it.

use crate::batches::{Order, BATCHES};
use crate::dcn::STAGES;
use crate::light::{SHORT_WATCH_FRAMES, WATCH_FRAMES};
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
/// stages undone after they ran, and fewer stages than all 16. They change only what `Light` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bringup { pub survey_order: bool, pub undo_after_stages: bool, pub stages_allowed: u8 }

impl Bringup {
    pub const RELEASE: Bringup = Bringup { survey_order: false, undo_after_stages: false, stages_allowed: STAGES };
    pub fn is_release(&self) -> bool { !self.survey_order && !self.undo_after_stages && self.stages_allowed >= STAGES }
}

impl fmt::Display for Bringup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_release() { return f.write_str("A RELEASE BUILD: NO BRING-UP SWITCH SET"); }
        f.write_str("A BRING-UP BUILD, NOT A RELEASE:")?;
        if self.survey_order { f.write_str(" SURVEY_ORDER")?; }
        if self.undo_after_stages { f.write_str(" UNDO_AFTER_STAGES")?; }
        if self.stages_allowed < STAGES { write!(f, " STAGES_ALLOWED {} OF {}", self.stages_allowed, STAGES)?; }
        f.write_str("; ONLY WHAT LIGHT DOES CHANGES")
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
}

impl Plan {
    /// Whether anything is written (step 2's I2C engine, or a stage): only then is a writer made.
    pub fn writes(&self) -> bool { self.edid || self.stages > 0 }
}

/// The plan for `setting`: `Off` and `Read` whatever the build; `Light` as boots 95 to 100 ran it on the Ryzen (11
/// batches, the EDID, 16 stages, HPD 1 followed, the standing check, the watch of 60), less what a bring-up switch takes.
pub fn plan(setting: Setting, b: Bringup) -> Plan {
    let order = if b.survey_order { Order::Survey } else { Order::Production };
    let none = Plan { setting, order, batches: 0, edid: false, stages: 0, undo_after_stages: false, hotplug: false, standing_check: false, watch_frames: WATCH_FRAMES };
    match setting {
        Setting::Off => none,
        Setting::Read => Plan { batches: BATCHES, ..none },
        Setting::Light => {
            let stages = b.stages_allowed.min(STAGES);
            // What follows the stages needs them in place, not undone; the standing check needs stage 15's picture.
            let follows = stages > 0 && !b.undo_after_stages;
            let standing_check = follows && stages >= 15;
            Plan { batches: BATCHES, edid: true, stages, undo_after_stages: b.undo_after_stages && stages > 0, hotplug: follows, standing_check,
                   watch_frames: if standing_check { SHORT_WATCH_FRAMES } else { WATCH_FRAMES }, ..none }
        }
    }
}
