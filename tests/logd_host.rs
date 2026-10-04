//! Host tests of logd's ring (logd/src/ring.rs): order and sequence numbers, wrap-around with dropped records,
//! clipping of text and names, and the rate limit per sender with the note about refused records.
#[path = "../logd/src/ring.rs"]
mod ring;

use ring::{Ring, RATE, SLOTS, TEXT};

#[test]
fn keeps_order_and_wraps() {
    let mut r = Box::new(Ring::new());
    assert_eq!((r.first(), r.next()), (0, 0));
    assert!(r.get(0).is_none());
    for i in 0..SLOTS as u64 + 10 {
        assert!(r.push(i * 20, 2 + i % 3, "vfs_server", 1, &format!("line {}\n", i)));
    }
    assert_eq!((r.first(), r.next(), r.dropped()), (10, SLOTS as u64 + 10, 10));
    assert!(r.get(9).is_none() && r.get(SLOTS as u64 + 10).is_none());
    let first = r.get(10).unwrap();
    assert_eq!((first.seq, first.text(), first.name(), first.pid, first.time_ms), (10, "line 10", "vfs_server", 3, 200));
    assert_eq!(r.get(SLOTS as u64 + 9).unwrap().text(), format!("line {}", SLOTS + 9));
}

#[test]
fn clips_text_and_names_on_character_boundaries() {
    let mut r = Ring::new();
    let long = "ж".repeat(150); // 300 bytes
    r.push(0, 7, "очень-длинное-имя", 9, &long);
    let record = r.get(0).unwrap();
    assert_eq!(record.text().len(), TEXT);
    assert_eq!(record.text(), "ж".repeat(100));
    assert_eq!(record.name(), "очень-дл", "16 bytes: 8 Cyrillic letters");
    assert_eq!(record.level, 3, "levels above error are errors");
    r.push(0, 7, "x", 1, "a\r\n");
    assert_eq!(r.get(1).unwrap().text(), "a");
}

#[test]
fn limits_each_sender() {
    let mut r = Ring::new();
    let mut kept = 0;
    for i in 0..RATE + 20 { if r.push(5_000 + i as u64, 42, "chatty", 1, "spam") { kept += 1; } }
    assert_eq!(kept, RATE);
    assert_eq!(r.suppressed(), 20);
    // Another sender is not affected.
    assert!(r.push(5_500, 43, "quiet", 1, "hello"));
    // In the next second the chatty one writes again, after a note of what was refused.
    assert!(r.push(6_000, 42, "chatty", 1, "again"));
    let n = r.next();
    let note = r.get(n - 2).unwrap();
    assert_eq!((note.text(), note.level, note.pid), (format!("20 records refused: more than {} a second", RATE).as_str(), 2, 42));
    assert_eq!(r.get(n - 1).unwrap().text(), "again");
    // Many senders: the table forgets the oldest, nothing is refused wrongly.
    for pid in 100..200 { assert!(r.push(7_000, pid, "many", 1, "one")); }
}
