// 000-KRN-0217: which services a start after an unfinished run leaves out, and when they start again (init/src/safe.rs).
#[path = "../init/src/safe.rs"]
mod safe;

use safe::*;

const A: u8 = 1;
const B: u8 = 2;
const RADIO: u8 = 1 << 0;
const SOUND: u8 = 1 << 2;

fn kept(slot: u8, starting: u8, suspected: u8) -> Option<Read> { Some(Read::Kept(Mark { slot, starting, suspected })) }

#[test]
fn a_run_that_ended_as_asked_leaves_nothing_out() {
    assert_eq!(decide(None, A, false), Decision { left_out: 0, named: 0, suspected: 0, why: Why::Ended });
    assert_eq!(decide(None, A, true).left_out, 0);
}

#[test]
fn a_cut_power_after_every_start_settled_is_a_normal_start() {
    // The DEXP, 2026-11-08: every service started, the network worked, the power cut before a minute past READY.
    assert_eq!(decide(kept(A, 0, 0), A, false), Decision { left_out: 0, named: 0, suspected: 0, why: Why::Nothing });
    assert_eq!(decide(kept(A, 0, RADIO), A, true).left_out, 0);
}

#[test]
fn a_run_cut_while_a_radio_started_suspects_it_and_a_second_leaves_it_out() {
    let once = decide(kept(A, RADIO | SOUND, 0), A, false);
    assert_eq!(once, Decision { left_out: 0, named: RADIO | SOUND, suspected: RADIO | SOUND, why: Why::Once });
    // The next run keeps the suspicion in its mark; cut again while only the radio was starting.
    let twice = decide(kept(A, RADIO, once.suspected), A, false);
    assert_eq!(twice, Decision { left_out: RADIO, named: RADIO, suspected: RADIO, why: Why::Twice });
    // A service starting for the first time is only suspected beside one left out.
    let mixed = decide(kept(A, RADIO | SOUND, RADIO), A, false);
    assert_eq!(mixed, Decision { left_out: RADIO, named: RADIO, suspected: RADIO | SOUND, why: Why::Twice });
}

#[test]
fn the_watchdogs_evidence_leaves_out_what_was_starting_at_once() {
    assert_eq!(decide(kept(B, RADIO | SOUND, 0), B, true), Decision { left_out: RADIO | SOUND, named: RADIO | SOUND, suspected: RADIO | SOUND, why: Why::Evidence });
}

#[test]
fn another_slots_run_leaves_nothing_out() {
    // A trial of slot B cut while its radio started, then slot A: A's radio is not B's to suspect (801 S3, S5).
    assert_eq!(decide(kept(B, RADIO, RADIO), A, true), Decision { left_out: 0, named: 0, suspected: 0, why: Why::OtherSlot(B) });
    assert_eq!(decide(kept(A, RADIO, RADIO), B, false).why, Why::OtherSlot(A));
}

#[test]
fn a_mark_of_another_version_or_damaged_is_read_as_unknown() {
    assert_eq!(Mark::read(&[1]), Read::Version(1));
    assert_eq!(Mark::read(&[VERSION, A]), Read::Damaged);
    assert_eq!(Mark::read(&[]), Read::Damaged);
    assert_eq!(decide(Some(Mark::read(&[1])), A, true), Decision { left_out: 0, named: 0, suspected: 0, why: Why::Unknown });
    let mark = Mark { slot: B, starting: RADIO, suspected: SOUND };
    assert_eq!(Mark::read(&mark.bytes()), Read::Kept(mark));
}

#[test]
fn a_start_settles_once_it_and_the_boot_ran_long_enough() {
    assert!(!settled(1_000, None, 1_000_000));
    assert!(!settled(1_000, Some(10_000), 10_000 + SETTLE_MS - 1));
    assert!(settled(1_000, Some(10_000), 10_000 + SETTLE_MS));
    assert!(!settled(200_000, Some(10_000), 200_000 + SETTLE_MS - 1));
    assert!(settled(200_000, Some(10_000), 200_000 + SETTLE_MS));
}

#[test]
fn the_left_out_come_back_one_at_a_time_radios_first() {
    assert_eq!((bit(0), bit(2), bit(8)), (RADIO, SOUND, 0));
    let radios = RADIO | 1 << 4 | 1 << 5;
    let ready = Some(10_000);
    assert_eq!(next_return(SOUND | 1 << 4, radios, false, ready, 10_000 + RETURN_MS - 1), None);
    assert_eq!(next_return(SOUND | 1 << 4, radios, false, ready, 10_000 + RETURN_MS), Some(4));
    assert_eq!(next_return(SOUND | 1 << 4, radios, true, ready, 10_000 + RETURN_MS), None);
    assert_eq!(next_return(SOUND, radios, false, ready, 10_000 + RETURN_MS), Some(2));
    assert_eq!(next_return(0, radios, false, ready, 10_000 + RETURN_MS), None);
    assert_eq!(next_return(SOUND, radios, false, None, 10_000 + RETURN_MS), None);
}
