//! Host tests of the loader's mark (bootloader/src/mark.rs, 000-KRN-0211): one line of a fixed size that the loader
//! overwrites in place, the boot count it carries, and what it refuses to read.
#[path = "../bootloader/src/mark.rs"]
mod mark;
use mark::*;

fn text(count: u32, stage: Stage) -> String { String::from_utf8(encode(count, stage).to_vec()).unwrap() }

#[test]
fn a_mark_is_one_line_of_its_size_and_reads_back() {
    for (count, stage) in [(1, Stage::Started), (42, Stage::Read), (u32::MAX, Stage::Handover)] {
        let line = encode(count, stage);
        assert_eq!(line.len(), LEN);
        assert_eq!(line[LEN - 1], b'\n');
        assert_eq!(text(count, stage).trim_end(), format!("MIND-REACHED 1 {count} {}", stage.name()));
        assert_eq!(mark::count(&line), Some(count));
    }
}

#[test]
fn the_next_boot_counts_one_more_and_never_zero() {
    assert_eq!(next(None), 1);
    assert_eq!(next(Some(7)), 8);
    assert_eq!(next(Some(u32::MAX)), 1);
}

#[test]
fn a_mark_of_another_shape_is_not_read() {
    for bad in [&b""[..], b"\n", b"MIND-REACHED 2 5 READ\n", b"MIND-REACHED 1 5\n", b"MIND-REACHED 1 x READ\n", b"MIND-REACHED 1 +5 READ\n", b"MIND-REACHED 1 4294967296 READ\n",
                b"MIND-REACHED 1 5 BOOTED\n", b"MIND-REACHED 1 5 READ more\n", b"OTHER 1 5 READ\n", &[0u8; LEN]] {
        assert_eq!(count(bad), None, "{:?}", String::from_utf8_lossy(bad));
    }
    // Only the first line is read: bytes after it, as a longer file of an older writer would leave, are ignored.
    let mut longer = encode(9, Stage::Read).to_vec();
    longer.extend_from_slice(b"junk\n");
    assert_eq!(count(&longer), Some(9));
}

#[test]
fn the_stages_are_named_as_boot_switch_reads_them() {
    // scripts/boot_switch.py's STAGES, in order.
    assert_eq!([Stage::Started, Stage::Read, Stage::Handover].map(Stage::name), ["STARTED", "READ", "HANDOVER"]);
    assert_eq!(FILE, "MIND\\REACHED");
    assert!(FILE.starts_with(DIR));
}
