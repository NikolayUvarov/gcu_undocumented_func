//! The session journal's file form (750-EFF-0006): written, read back, a cut last line dropped, corruption refused.
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
    let (read, n) = journal::read(text.as_bytes(), "mind-test").unwrap();
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
