//! `mode` (750-EFF-0013, `init.wit` 1.5): the machine's modes, which decide whether `init` runs the Effector agent's
//! services. `mode` shows them and who set them last; `mode development|maintenance on|off [reason]` turns one on after the
//! operator agrees at the local console, or off at once, through `init`'s client badged `BADGE_MODE` in `SLOT_MODE`
//! (ABI 14, 750-KRN-0143). `init` writes `data/mode.conf` and starts or stops the services itself. This file is the
//! command line and the text, host-tested; `modectl.rs` makes the calls.
use core::fmt::Write;

/// The two modes of `init.wit` 1.5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which { Development, Maintenance }

pub const USAGE: &str = "USAGE: MODE [DEVELOPMENT|MAINTENANCE ON|OFF [REASON]]";
/// The longest reason `set-mode` carries (`string<120>`).
pub const REASON_MAX: usize = 120;

/// What `mode` was asked: the modes shown, or one turned on or off, with the operator's reason (empty: `operator`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked<'a> { Show, Set { which: Which, on: bool, reason: &'a str } }

pub fn parse(args: &str) -> Result<Asked<'_>, &'static str> {
    let args = args.trim();
    if args.is_empty() { return Ok(Asked::Show); }
    let (which, rest) = args.split_once(char::is_whitespace).unwrap_or((args, ""));
    let which = if which.eq_ignore_ascii_case("development") { Which::Development } else if which.eq_ignore_ascii_case("maintenance") { Which::Maintenance } else { return Err(USAGE) };
    let rest = rest.trim_start();
    let (word, reason) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let on = if word.eq_ignore_ascii_case("on") { true } else if word.eq_ignore_ascii_case("off") { false } else { return Err(USAGE) };
    let reason = reason.trim();
    if reason.len() > REASON_MAX { return Err("MODE: THE REASON TAKES AT MOST 120 BYTES"); }
    Ok(Asked::Set { which, on, reason })
}

pub fn name(which: Which) -> &'static str { match which { Which::Development => "DEVELOPMENT", Which::Maintenance => "MAINTENANCE" } }

/// `MODE: NORMAL (NO EFFECTOR SERVICES), SET BY disk` or `MODE: MAINTENANCE, SET BY operator: <reason>`, from
/// `mode-state`'s fields.
pub fn line(out: &mut impl Write, development: bool, maintenance: bool, by: &str, reason: &str) {
    let on = match (development, maintenance) {
        (false, false) => "NORMAL (NO EFFECTOR SERVICES)",
        (true, false) => "DEVELOPMENT",
        (false, true) => "MAINTENANCE",
        (true, true) => "DEVELOPMENT AND MAINTENANCE",
    };
    let _ = write!(out, "MODE: {}, SET BY {}", on, by);
    let _ = if reason.is_empty() { writeln!(out) } else { writeln!(out, ": {}", reason) };
}

/// The question before a mode goes on: what the Effector agent may then do, and that it lasts.
pub fn question(out: &mut impl Write, which: Which) {
    let _ = write!(out, "TURN THE {} MODE ON? INIT STARTS THE EFFECTOR AGENT'S SERVICES WHERE THIS BUILD HAS THEM, AND A REMOTE SESSION MAY ACT ON THIS \
                         MACHINE AS DATA/EFFECTOR/POLICY ALLOWS; THE MODE STAYS ON ACROSS REBOOTS UNTIL MODE {} OFF.", name(which), name(which));
}

/// The line above the first prompt while a mode is on (750-EFF-0013: "while a mode is on, the screen says so"); false in
/// the normal mode, which says nothing.
pub fn notice(out: &mut impl Write, development: bool, maintenance: bool, by: &str) -> bool {
    let (on, off) = match (development, maintenance) {
        (false, false) => return false,
        (true, false) => ("THE DEVELOPMENT MODE IS", "MODE DEVELOPMENT OFF"),
        (false, true) => ("THE MAINTENANCE MODE IS", "MODE MAINTENANCE OFF"),
        (true, true) => ("THE DEVELOPMENT AND MAINTENANCE MODES ARE", "MODE … OFF"),
    };
    let _ = writeln!(out, "{} ON (SET BY {}): THE EFFECTOR AGENT MAY ACT ON THIS MACHINE. {} ENDS IT.", on, by, off);
    true
}
