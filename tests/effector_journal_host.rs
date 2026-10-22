//! The session journal's file form (750-EFF-0006): written, read back, a cut last line dropped, corruption refused;
//! `done` lines keep acknowledged answers from being acknowledged again; a full journal written again whole.
#![allow(dead_code)]
#[path = "../libmind/src/sha256.rs"] mod sha256;
#[path = "../effector/src/session.rs"] mod session;
#[path = "../effector/src/journal.rs"] mod journal;
use session::*;

fn record(id: &str, result: Final, expires: Option<u64>) -> Record {
    Record { agent: Id::new("mind-test").unwrap(), id: Id::command(id).unwrap(), identity: [0xab; 32], result, expires_unix: expires }
}

fn file(records: &[Record]) -> String {
    let mut text = String::from(journal::HEADER);
    for r in records {
        let mut out = [0u8; journal::LINE_MAX];
        let n = journal::line(r, &mut out);
        text.push_str(std::str::from_utf8(&out[..n]).unwrap());
    }
    text
}

#[test]
fn records_written_are_read_back() {
    let records = [record("20261021064900.000000001_000042", Final::Denied, None), record("cmd-2", Final::Expired, Some(1_792_566_386)), record(&"x".repeat(64), Final::Denied, Some(0))];
    let text = file(&records);
    assert!(text.lines().nth(1).unwrap().ends_with(" denied -"));
    let (read, n, _) = journal::read(text.as_bytes(), "mind-test").unwrap();
    assert_eq!(n, 3);
    assert_eq!(read[..3].iter().map(|r| r.unwrap()).collect::<Vec<_>>(), records);
    // The session takes them back and acknowledges them again, never runs them.
    let s = Session::new("mind-test", &records, 0).unwrap();
    assert_eq!(s.record_count(), 3);
}

#[test]
fn an_empty_file_and_a_cut_last_line() {
    assert_eq!(journal::read(b"", "mind-test").unwrap().1, 0);
    let text = file(&[record("cmd-1", Final::Denied, None)]);
    let cut = format!("{text}cmd-2 abab");
    assert_eq!(journal::read(cut.as_bytes(), "mind-test").unwrap().1, 1, "the cut line was never acknowledged");
}

#[test]
fn corruption_is_refused() {
    let good = file(&[record("cmd-1", Final::Denied, None)]);
    let line1 = good.lines().nth(1).unwrap().to_owned();
    for bad in [
        format!("effector-journal 2\n{line1}\n"),
        format!("{line1}\n"),
        format!("{}cmd-2 {} denied -\n", journal::HEADER, "zz".repeat(32)),
        format!("{}cmd-2 {} done -\n", journal::HEADER, "ab".repeat(32)),
        format!("{}cmd-2 {} denied 01\n", journal::HEADER, "ab".repeat(32)),
        format!("{}cmd-2 {} denied - extra\n", journal::HEADER, "ab".repeat(32)),
        format!("{}bad/id {} denied -\n", journal::HEADER, "ab".repeat(32)),
        format!("{good}{line1}\n"),
    ] {
        assert_eq!(journal::read(bad.as_bytes(), "mind-test").err(), Some(Error::Journal), "{bad:?}");
    }
    let full: Vec<Record> = (0..=RECORDS_MAX).map(|i| record(&format!("c{i}"), Final::Denied, None)).collect();
    assert_eq!(journal::read(file(&full).as_bytes(), "mind-test").err(), Some(Error::Full));
    assert_eq!(journal::read(good.as_bytes(), "bad.agent").err(), Some(Error::Identifier));
}

#[test]
fn an_action_s_results_are_read_back_and_an_unknown_one_refused() {
    let records = [record("a", Final::Completed, None), record("b", Final::Failed(Failure::NoPolicy), Some(5)), record("c", Final::Failed(Failure::Unsupported), None)];
    let text = file(&records);
    assert!(text.contains(" completed -\n") && text.contains(" failed-no-policy 5\n") && text.contains(" failed-unsupported -\n"), "{text}");
    let (read, n, _) = journal::read(text.as_bytes(), "mind-test").unwrap();
    assert_eq!(n, 3);
    for (r, w) in read.iter().flatten().zip(records.iter()) { assert_eq!(r, w); }
    let bad = text.replace("failed-no-policy", "failed-gremlins");
    assert_eq!(journal::read(bad.as_bytes(), "mind-test").err(), Some(Error::Journal));
}

#[test]
fn done_lines_and_artifacts() {
    let records = [record("a", Final::Captured(Artifact::Screen), None), record("b", Final::Captured(Artifact::Log), Some(9)), record("c", Final::Denied, None)];
    let mut text = file(&records);
    assert!(text.contains(" captured-screen -\n") && text.contains(" captured-log 9\n"), "{text}");
    let mut line = [0u8; journal::LINE_MAX];
    let length = journal::done_line(&records[1], &mut line);
    let done_b = std::str::from_utf8(&line[..length]).unwrap().to_owned();
    text.push_str(&done_b);
    assert!(text.ends_with(&format!("b {} done -\n", "ab".repeat(32))), "{text}");
    let (read, n, done) = journal::read(text.as_bytes(), "mind-test").unwrap();
    assert_eq!((n, &done[..3]), (3, &[false, true, false][..]));
    assert_eq!(read[0].unwrap().result, Final::Captured(Artifact::Screen));
    // The session takes a done record back without acknowledging it again.
    let s = Session::restore("mind-test", &records, &done[..3], 0).unwrap();
    assert!(s.busy(), "a and c still wait for their ACK");
    assert_eq!(s.journal().map(|e| e.1).collect::<Vec<_>>(), [false, true, false]);
    // Written again whole (each done line after its record), it reads back the same.
    let mut again = Vec::new();
    assert!(journal::write_all(s.journal(), &mut |b| { again.extend_from_slice(b); true }));
    let (reread, m, redone) = journal::read(&again, "mind-test").unwrap();
    assert_eq!((m, &reread[..3], &redone[..3]), (3, &read[..3], &done[..3]));
    let twice = format!("{text}{done_b}");
    assert_eq!(journal::read(twice.as_bytes(), "mind-test").unwrap().2[..3], [false, true, false], "a redelivered answer's second done line");
    let other = done_b.replace(&"ab".repeat(32), &"cd".repeat(32));
    for bad in [format!("{text}{other}"), text.replace(" done -", " done 5"), text.replace("captured-log", "captured-video")] {
        assert_eq!(journal::read(bad.as_bytes(), "mind-test").err(), Some(Error::Journal), "{bad:?}");
    }
}
