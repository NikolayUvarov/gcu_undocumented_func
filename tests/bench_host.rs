//! Host tests of the tables and statistics of check, bench and kbench (bench/src/report.rs, main task 176).
#![allow(dead_code)]
extern crate alloc;
#[path = "../bench/src/report.rs"]
mod report;

use report::*;

#[test]
fn statistics_of_samples() {
    let mut samples: Vec<u64> = (1..=100).rev().collect();
    let s = Stats::of(&mut samples);
    assert_eq!((s.count, s.min, s.max, s.mean), (100, 1, 100, 50));
    assert_eq!(s.median, 50); // (50 + 51) / 2
    assert_eq!(s.p99, 99); // the 99th of 100 by nearest rank
    assert_eq!(samples[0], 1, "sorted in place");
    assert_eq!(s.histogram.iter().map(|&(_, n)| n).sum::<usize>(), 100);
    assert_eq!(s.histogram.first(), Some(&(1, 1)));
    assert_eq!(s.histogram.last(), Some(&(7, 37))); // 64..=100
    assert_eq!(Stats::of(&mut [7]).p99, 7);
    assert_eq!(Stats::of(&mut []), Stats::default());
    let zeros = Stats::of(&mut [0, 0, 3]);
    assert_eq!(zeros.histogram, vec![(0, 2), (2, 1)]);
}

#[test]
fn units_have_three_significant_digits() {
    assert_eq!(duration(0), "0 ns");
    assert_eq!(duration(999), "999 ns");
    assert_eq!(duration(1_000), "1.00 µs");
    assert_eq!(duration(1_826), "1.82 µs");
    assert_eq!(duration(181_211), "181 µs");
    assert_eq!(duration(13_100_000), "13.1 ms");
    assert_eq!(duration(1_200_000_000), "1.20 s");
    assert_eq!(rate(1 << 20, 1_000_000_000), "1.00 MiB/s");
    assert_eq!(rate(1 << 20, 750_000), "1.30 GiB/s");
    assert_eq!(rate(100, 1_000_000_000), "100 B/s");
    assert_eq!(bytes(512), "512 B");
    assert_eq!(bytes(64 << 20), "64.0 MiB");
    assert_eq!(bytes(7_768 << 20), "7.58 GiB");
    for ns in [1u64, 999, 1000, 99_999, 999_999_999, 999_999_999_999] { assert!(duration(ns).chars().count() <= 8, "{}", duration(ns)); }
}

#[test]
fn bars_are_log_scaled_in_eighths() {
    assert_eq!(log2_eighths(0), 0);
    assert_eq!(log2_eighths(8), 24);
    assert_eq!(log2_eighths(12), 28); // 8 * 1.5: half an octave
    assert_eq!(bar(0, 4), "    ");
    assert_eq!(bar(1, 4), "▏   "); // below the scale: the least visible bar
    assert_eq!(bar(1 << 30, 4), "████");
    assert_eq!(bar(u64::MAX, 4), "████");
    let (short, long) = (bar(1_000, 15), bar(1_000_000, 15));
    assert!(short.trim_end().chars().count() < long.trim_end().chars().count());
    for ns in [5u64, 100, 10_000, 1 << 20, 1 << 40] { assert_eq!(bar(ns, 15).chars().count(), 15); }
    assert_eq!(share(1, 2, 4), "██  ");
    assert_eq!(share(1, 2, 1), "▌");
    assert_eq!(share(1, 16, 1), " "); // half an eighth
    assert_eq!(share(5, 0, 2), "██");
}

#[test]
fn tables_fit_and_cut() {
    let table = Table::new(&[(24, Align::Left), (8, Align::Right), (15, Align::Left), (8, Align::Right), (8, Align::Right)]);
    assert_eq!(table.width(), 79);
    assert!(table.width() <= WIDTH);
    for line in [table.top(), table.middle(), table.bottom(), table.row(&["a", "b"]), table.row(&["x"; 5])] {
        assert_eq!(line.chars().count(), 79, "{}", line);
    }
    assert!(table.top().starts_with('┌') && table.top().ends_with('┐') && table.top().contains('┬'));
    let row = table.row(&["IPC between two processes here", "181 µs", "███", "177 µs", "460 µs"]);
    assert!(row.starts_with("│ IPC between two process… │   181 µs │ ███"), "{}", row);
    assert_eq!(fit("abc", 3), "abc");
    assert_eq!(fit("abcd", 3), "ab…");
    assert_eq!(fit("abcd", 0), "");
}

#[test]
fn log_lines_and_verdicts() {
    let s = Stats::of(&mut [1_100, 1_200, 2_500]);
    let lines = stats_lines("call and reply", "ns", &s);
    assert_eq!(lines[0], "call and reply (3 samples, ns)");
    assert!(lines[1].contains("min 1100 ns") && lines[1].contains("max 2500 ns"), "{}", lines[1]);
    assert_eq!(lines.len(), 3 + s.histogram.len());
    assert!(lines[3].contains("1.02 µs") && lines[3].ends_with(" 2"), "{}", lines[3]);
    let verdicts = [Verdict::Passed("ok".into()), Verdict::Failed("why".into()), Verdict::Skipped("no device".into()), Verdict::Passed(String::new())];
    assert_eq!(tally(&verdicts), (2, 1, 1));
    assert_eq!(verdicts.iter().map(Verdict::mark).collect::<String>(), "✓✗○✓");
    assert_eq!(verdicts[2].detail(), "no device");
    assert_eq!(verdicts[1].word(), "FAIL");
}

#[test]
fn the_boot_number_is_the_highest_log() {
    let names = ["boot0001.log", "boot0012.log", "hw0012.txt", "boot0003.log", "bootxx.log", "kbench0012.txt"];
    assert_eq!(highest_number(names.iter().copied(), "boot", ".log"), 12);
    assert_eq!(highest_number(["readme.txt"].iter().copied(), "boot", ".log"), 0);
}
