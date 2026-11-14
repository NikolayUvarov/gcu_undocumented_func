//! `mode` (750-EFF-0013, `init.wit` 1.5): the machine's modes, which decide whether `init` runs the Effector agent's
//! services. `mode` shows them and who set them last; `mode development|maintenance on|off [reason]` turns one on after the
//! operator agrees at the local console, or off at once, through `init`'s client badged `BADGE_MODE` in `SLOT_MODE`
//! (ABI 14, 750-KRN-0143). `init` writes `data/mode.conf` and starts or stops the services itself. Since `init.wit` 1.11
//! (801-APP-0139, 751-KRN-0185) also the owner's permissions `remote-update` (a release from the Effector server may be
//! installed) and `link-remote-update` (the link's console may switch the first). This file is the command line and the
//! text, host-tested; `modectl.rs` makes the calls.
use core::fmt::Write;

/// The two modes of `init.wit` 1.5 and the two permissions of 1.11.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which { Development, Maintenance, RemoteUpdate, LinkRemoteUpdate }

pub const USAGE: &str = "USAGE: MODE [DEVELOPMENT|MAINTENANCE|REMOTE-UPDATE|LINK-REMOTE-UPDATE ON|OFF [REASON]]";
/// The longest reason `set-mode` carries (`string<120>`).
pub const REASON_MAX: usize = 120;

/// What `mode` was asked: the modes shown, or one turned on or off, with the operator's reason (empty: `operator`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked<'a> { Show, Set { which: Which, on: bool, reason: &'a str } }

pub fn parse(args: &str) -> Result<Asked<'_>, &'static str> {
    let args = args.trim();
    if args.is_empty() { return Ok(Asked::Show); }
    let (which, rest) = args.split_once(char::is_whitespace).unwrap_or((args, ""));
    let which = [("development", Which::Development), ("maintenance", Which::Maintenance), ("remote-update", Which::RemoteUpdate), ("link-remote-update", Which::LinkRemoteUpdate)]
        .into_iter().find(|(word, _)| which.eq_ignore_ascii_case(word)).map(|(_, which)| which).ok_or(USAGE)?;
    let rest = rest.trim_start();
    let (word, reason) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let on = if word.eq_ignore_ascii_case("on") { true } else if word.eq_ignore_ascii_case("off") { false } else { return Err(USAGE) };
    let reason = reason.trim();
    if reason.len() > REASON_MAX { return Err("MODE: THE REASON TAKES AT MOST 120 BYTES"); }
    Ok(Asked::Set { which, on, reason })
}

pub fn name(which: Which) -> &'static str {
    match which { Which::Development => "DEVELOPMENT", Which::Maintenance => "MAINTENANCE", Which::RemoteUpdate => "REMOTE-UPDATE", Which::LinkRemoteUpdate => "LINK-REMOTE-UPDATE" }
}

/// Whether the link's console (800-APP-0108) may have this line run: only `remote-update on|off`, and only while init
/// reports the owner's `link-remote-update` (751-KRN-0185); every other `mode` from the link is refused.
pub fn link_may(asked: &Asked<'_>, link_remote_update: bool) -> bool {
    matches!(asked, Asked::Set { which: Which::RemoteUpdate, .. }) && link_remote_update
}

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

/// The owner's two permissions, as `mode-state` says whether each counts now (`init.wit` 1.11).
pub fn permissions(out: &mut impl Write, remote_update: bool, link_remote_update: bool) {
    let _ = writeln!(out, "REMOTE UPDATE: {}; THE LINK'S CONSOLE MAY SWITCH IT: {}",
        if remote_update { "ON (A RELEASE SENT THROUGH THE EFFECTOR SERVER MAY BE INSTALLED)" } else { "OFF" }, if link_remote_update { "YES" } else { "NO" });
}

/// The question before a mode or a permission goes on: what it then allows, and that it lasts.
pub fn question(out: &mut impl Write, which: Which) {
    let _ = match which {
        Which::Development | Which::Maintenance => write!(out, "TURN THE {} MODE ON? INIT STARTS THE EFFECTOR AGENT'S SERVICES WHERE THIS BUILD HAS THEM, AND A REMOTE SESSION MAY ACT ON THIS \
                         MACHINE AS DATA/EFFECTOR/POLICY ALLOWS; THE MODE STAYS ON ACROSS REBOOTS UNTIL MODE {} OFF.", name(which), name(which)),
        Which::RemoteUpdate => write!(out, "TURN REMOTE-UPDATE ON? A RELEASE SENT THROUGH THE EFFECTOR SERVER MAY THEN BE INSTALLED ON THIS MACHINE AND THE MACHINE \
                         RESTARTED INTO IT, AS DATA/EFFECTOR/POLICY ALLOWS; IT COUNTS ONLY WITH THE DEVELOPMENT OR MAINTENANCE MODE AND STAYS ON \
                         ACROSS REBOOTS UNTIL MODE REMOTE-UPDATE OFF."),
        Which::LinkRemoteUpdate => write!(out, "TURN LINK-REMOTE-UPDATE ON? THE LINK'S CONSOLE (THE CABLE) MAY THEN TURN REMOTE-UPDATE ON OR OFF; IT COUNTS ONLY WITH \
                         THE LINK'S CONSOLE AND STAYS ON ACROSS REBOOTS UNTIL MODE LINK-REMOTE-UPDATE OFF."),
    };
}

/// The line above the first prompt while `remote-update` counts (EFF's request of 2026-10-29), with init's warning when
/// the slot trusts the public test key (751-KRN-0185); false while it does not count.
pub fn remote_notice(out: &mut impl Write, remote_update: bool, link_remote_update: bool, test_key: bool) -> bool {
    if !remote_update { return false; }
    let _ = writeln!(out, "REMOTE UPDATE ON: A RELEASE SENT THROUGH THE EFFECTOR SERVER MAY BE INSTALLED{}; THE LINK'S CONSOLE {} SWITCH IT. MODE REMOTE-UPDATE OFF ENDS IT.",
        if test_key { " (THE TEST KEYS: ANY RELEASE FROM THE SERVER IS ACCEPTED)" } else { "" }, if link_remote_update { "MAY" } else { "MAY NOT" });
    true
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
