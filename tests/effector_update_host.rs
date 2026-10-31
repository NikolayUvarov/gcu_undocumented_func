//! A developer's release across its restart (751-EFF-0023): the command kept in the private `update` file before the
//! apply and read back after it; the answer from the updater's view: updated and confirmed, waiting on trial, fallen
//! back, or no restart at all.
#![allow(dead_code)]
extern crate alloc;
#[path = "../effector/src/update.rs"] mod update;
use update::*;

fn pending() -> Pending {
    Pending {
        id: "20261030120000.000000001_000042".into(), agent_id: "mind_09dca8b4def0f6e6".into(), action: "update_agent".into(), program: "MIND Core".into(),
        payload: r#"{"size":4096,"sha256":"aa","tries":1}"#.into(), expires_unix: Some(1_793_000_000), received_unix: Some(1_792_999_000),
        at_ms: 400_000, boot_unix: Some(1_792_998_700), running: 202610290937, staged: 202610301200,
    }
}

fn view(slot: u8, running: u64, recorded: u8, confirmed: bool, failure: &str) -> View<'_> {
    View { slot, trial: slot != 0, running, recorded, confirmed, staged: 0, failure }
}

#[test]
fn the_command_is_kept_whole_across_the_restart() {
    let p = pending();
    let text = write(&p).unwrap();
    assert!(text.starts_with(HEADER) && text.ends_with("payload {\"size\":4096,\"sha256\":\"aa\",\"tries\":1}\n"), "{text}");
    assert_eq!(read(text.as_bytes()), Some(p.clone()));
    // A payload with a newline or the key's own text stays whole: it is the rest of the file.
    let odd = Pending { payload: "line one\npayload two".into(), program: String::new(), expires_unix: None, received_unix: None, boot_unix: None, ..p.clone() };
    assert_eq!(read(write(&odd).unwrap().as_bytes()), Some(odd));
    assert_eq!(write(&Pending { program: "two\nlines".into(), ..p.clone() }), None);
    // Cut, or not this file's form: not read.
    assert_eq!(read(&text.as_bytes()[..text.len() - 1]), None);
    assert_eq!(read(text.replace("running 202610290937", "running x").as_bytes()), None);
    assert_eq!(read(b"effector-journal 1\n"), None);
}

#[test]
fn a_restart_is_a_lower_uptime_or_another_boot_time() {
    let p = pending();
    assert!(restarted(&p, 30_000, None), "the uptime started again");
    assert!(!restarted(&p, 500_000, Some(1_792_998_701)), "the same boot, the agent alone restarted");
    assert!(restarted(&p, 500_000, Some(1_792_999_400)), "a later boot that took longer than the first one's uptime");
    assert!(!restarted(&p, 500_000, None), "no clock: the uptime decides");
}

#[test]
fn the_answer_follows_the_updaters_view() {
    let p = pending();
    let answer = |v: Verdict| match v { Verdict::Answer { exit_code, stdout, error } => (exit_code, stdout, error), Verdict::Wait => panic!("waits") };
    assert_eq!(answer(verdict(&p, true, Some(&view(b'B', p.staged, b'B', true, "")), 3000)), (0, "updated 202610301200, slot B confirmed".into(), ""));
    // On trial, not yet confirmed: it waits, up to the kernel's deadline and the updater's margin.
    assert_eq!(verdict(&p, true, Some(&view(b'B', p.staged, b'B', false, "")), 3000), Verdict::Wait);
    assert_eq!(answer(verdict(&p, true, Some(&view(b'B', p.staged, b'B', false, "")), CONFIRM_WAIT_MS)).2, "not-confirmed");
    // Back in the old slot: the trial failed and the kernel fell back.
    let (code, stdout, error) = answer(verdict(&p, true, Some(&view(b'A', p.running, b'B', false, "apply: Trial")), 3000));
    assert_eq!((code, error), (1, "fell-back"));
    assert_eq!(stdout, "fell back: slot A runs version 202610290937, not 202610301200; the updater: apply: Trial");
    // No restart since the apply: the agent alone restarted.
    assert_eq!(answer(verdict(&p, false, Some(&view(b'A', p.running, b'B', false, "")), 0)).2, "restart");
    // No view yet: it waits, then says so.
    assert_eq!(verdict(&p, true, None, 1000), Verdict::Wait);
    assert_eq!(answer(verdict(&p, true, None, CONFIRM_WAIT_MS)).2, "io");
}
