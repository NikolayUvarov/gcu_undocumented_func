//! A developer's release across the restart it asks for (751-EFF-0023): once the updater staged it, the command is kept
//! in the private `update` file, flushed before the release is applied; after the restart the agent answers it from the
//! updater's view (`update-state`): updated and confirmed, still on trial (it waits), or fallen back. The answer is
//! journaled before its ACK as every answer, then the file goes.
use alloc::{format, string::String};

pub const HEADER: &str = "effector-update 1\n";
pub const FILE_MAX: usize = 2048;
/// How long a trial boot of the staged version may stay unconfirmed: past the kernel's deadline (120 s) and the
/// updater's margin, the machine has restarted into the fallback or the updater has confirmed it.
pub const CONFIRM_WAIT_MS: u64 = 180_000;

/// The command kept across the restart, as it came (its journal identity is computed again from it), and what the
/// agent knew when it asked for the apply: the uptime and the boot's wall time, the running and staged versions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    pub id: String, pub agent_id: String, pub action: String, pub program: String, pub payload: String,
    pub expires_unix: Option<u64>, pub received_unix: Option<u64>,
    pub at_ms: u64, pub boot_unix: Option<u64>, pub running: u64, pub staged: u64,
}

/// The updater's view as the gateway gives it (`effector.wit` 1.5 `update-state`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct View<'a> { pub slot: u8, pub trial: bool, pub running: u64, pub recorded: u8, pub confirmed: bool, pub staged: u64, pub failure: &'a str }

/// What to answer: nothing yet, or a result with its exit status, `stdout` and error name (empty when completed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict { Wait, Answer { exit_code: u64, stdout: String, error: &'static str } }

fn optional(v: Option<u64>) -> String { v.map_or(String::from("-"), |v| format!("{}", v)) }

/// The file's text; `None` if a field would break its lines.
pub fn write(p: &Pending) -> Option<String> {
    if [&p.id, &p.agent_id, &p.action, &p.program].iter().any(|f| f.contains('\n')) { return None; }
    let text = format!("{}id {}\nagent {}\naction {}\nprogram {}\nexpires {}\nreceived {}\nat {}\nboot {}\nrunning {}\nstaged {}\npayload {}\n",
        HEADER, p.id, p.agent_id, p.action, p.program, optional(p.expires_unix), optional(p.received_unix), p.at_ms, optional(p.boot_unix),
        p.running, p.staged, p.payload);
    (text.len() <= FILE_MAX).then_some(text)
}

/// The file read back; `None` if it is not one `write` made whole.
pub fn read(bytes: &[u8]) -> Option<Pending> {
    let text = core::str::from_utf8(bytes).ok()?.strip_prefix(HEADER)?;
    let (head, payload) = text.split_once("\npayload ")?;
    let payload = payload.strip_suffix('\n')?;
    let mut lines = head.lines();
    let mut field = |key: &str| lines.next().and_then(|l| l.strip_prefix(key)).and_then(|l| l.strip_prefix(' ')).map(String::from);
    let (id, agent_id, action, program) = (field("id")?, field("agent")?, field("action")?, field("program")?);
    let number = |s: String| s.parse::<u64>().ok();
    let optional = |s: String| if s == "-" { Some(None) } else { s.parse::<u64>().ok().map(Some) };
    let (expires_unix, received_unix) = (optional(field("expires")?)?, optional(field("received")?)?);
    let (at_ms, boot_unix) = (number(field("at")?)?, optional(field("boot")?)?);
    let (running, staged) = (number(field("running")?)?, number(field("staged")?)?);
    if id.is_empty() || lines.next().is_some() { return None; }
    Some(Pending { id, agent_id, action, program, payload: String::from(payload), expires_unix, received_unix, at_ms, boot_unix, running, staged })
}

/// Whether the machine restarted since the apply was asked: its uptime is below the one then, or it booted at another
/// wall time.
pub fn restarted(p: &Pending, now_ms: u64, boot_unix: Option<u64>) -> bool {
    now_ms < p.at_ms || matches!((p.boot_unix, boot_unix), (Some(a), Some(b)) if a.abs_diff(b) > 5)
}

fn slot(s: u8) -> char { if s == 0 { '-' } else { s as char } }

/// The answer from the updater's view (`None`: it could not be read), `waited_ms` after the agent started.
pub fn verdict(p: &Pending, restarted: bool, view: Option<&View<'_>>, waited_ms: u64) -> Verdict {
    let answer = |exit_code, stdout: String, error| Verdict::Answer { exit_code, stdout, error };
    if !restarted { return answer(1, format!("the restart did not come: version {} stays staged", p.staged), "restart"); }
    let Some(v) = view else {
        return if waited_ms < CONFIRM_WAIT_MS { Verdict::Wait } else { answer(1, String::from("the updater's state could not be read"), "io") };
    };
    if v.running == p.staged && v.slot != 0 {
        if v.recorded == v.slot && v.confirmed { return answer(0, format!("updated {}, slot {} confirmed", p.staged, slot(v.slot)), ""); }
        return if waited_ms < CONFIRM_WAIT_MS { Verdict::Wait }
            else { answer(1, format!("version {} runs in slot {} on trial, not confirmed", p.staged, slot(v.slot)), "not-confirmed") };
    }
    let why = if v.failure.is_empty() { String::new() } else { format!("; the updater: {}", v.failure) };
    answer(1, format!("fell back: slot {} runs version {}, not {}{}", slot(v.slot), v.running, p.staged, why), "fell-back")
}
