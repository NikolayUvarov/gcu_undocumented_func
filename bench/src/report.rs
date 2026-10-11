//! Statistics, units, bars and tables of `check`, `bench` and `kbench` (main task 176). Pure: tested on the host
//! (tests/bench_host.rs).
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The widest table on the screen, in columns: below 80, so an 80-column console does not wrap it.
pub const WIDTH: usize = 79;

/// One measurement's samples summed up, in the samples' unit (nanoseconds for durations).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub count: usize,
    pub min: u64,
    pub median: u64,
    pub mean: u64,
    pub p99: u64,
    pub max: u64,
    /// (k, n): n samples in [2^(k-1), 2^k), k = 0 for zero; ascending k, empty buckets left out.
    pub histogram: Vec<(u32, usize)>,
}

impl Stats {
    /// Sorts `samples` and sums them up; all zero for none.
    pub fn of(samples: &mut [u64]) -> Self {
        let n = samples.len();
        if n == 0 { return Self::default(); }
        samples.sort_unstable();
        let mean = (samples.iter().map(|&s| s as u128).sum::<u128>() / n as u128) as u64;
        let median = ((samples[(n - 1) / 2] as u128 + samples[n / 2] as u128) / 2) as u64;
        let p99 = samples[(n * 99).div_ceil(100) - 1];
        let mut histogram: Vec<(u32, usize)> = Vec::new();
        for &s in samples.iter() {
            let k = 64 - s.leading_zeros();
            match histogram.last_mut() { Some((last, count)) if *last == k => *count += 1, _ => histogram.push((k, 1)) }
        }
        Self { count: n, min: samples[0], median, mean, p99, max: samples[n - 1], histogram }
    }
}

/// The lower bound of histogram bucket `k`.
pub fn bucket_floor(k: u32) -> u64 { if k == 0 { 0 } else { 1u64 << (k - 1) } }

/// Three significant digits: 123 ns, 4.56 µs, 78.9 ms, 1.20 s (at most 8 columns below 1000 s).
pub fn duration(ns: u64) -> String {
    const UNITS: [(u64, &str); 3] = [(1_000_000_000, "s"), (1_000_000, "ms"), (1_000, "µs")];
    match UNITS.iter().find(|(scale, _)| ns >= *scale) {
        Some(&(scale, unit)) => scaled(ns as u128 * 100 / scale as u128, unit),
        None => format!("{} ns", ns),
    }
}

/// Bytes a second from `bytes` moved in `ns`: 812 KiB/s, 1.25 GiB/s.
pub fn rate(bytes: u64, ns: u64) -> String {
    let per_second = bytes as u128 * 1_000_000_000 / ns.max(1) as u128;
    const UNITS: [(u128, &str); 3] = [(1 << 30, "GiB/s"), (1 << 20, "MiB/s"), (1 << 10, "KiB/s")];
    match UNITS.iter().find(|(scale, _)| per_second >= *scale) {
        Some(&(scale, unit)) => scaled(per_second * 100 / scale, unit),
        None => format!("{} B/s", per_second),
    }
}

/// Bytes with a binary unit: 512 B, 4.00 KiB, 7.59 GiB.
pub fn bytes(n: u64) -> String {
    const UNITS: [(u64, &str); 3] = [(1 << 30, "GiB"), (1 << 20, "MiB"), (1 << 10, "KiB")];
    match UNITS.iter().find(|(scale, _)| n >= *scale) {
        Some(&(scale, unit)) => scaled(n as u128 * 100 / scale as u128, unit),
        None => format!("{} B", n),
    }
}

// A value given in hundredths, with three significant digits.
fn scaled(hundredths: u128, unit: &str) -> String {
    match hundredths {
        0..=999 => format!("{}.{:02} {}", hundredths / 100, hundredths % 100, unit),
        1000..=9999 => format!("{}.{} {}", hundredths / 100, hundredths / 10 % 10, unit),
        _ => format!("{} {}", hundredths / 100, unit),
    }
}

/// log2 of `v` in eighths (the octave and the next three bits), 0 for 0 and 1.
pub fn log2_eighths(v: u64) -> u32 {
    if v <= 1 { return 0; }
    let k = 63 - v.leading_zeros();
    let fraction = if k >= 3 { (v >> (k - 3)) & 7 } else { (v << (3 - k)) & 7 };
    k * 8 + fraction as u32
}

/// The shortest and the longest duration a bar shows: 8 ns and 2^30 ns (1.07 s), on a log scale.
const BAR_FROM: u32 = 3 * 8;
const BAR_TO: u32 = 30 * 8;

/// A bar `cells` wide whose length is `ns` on a log scale from 8 ns to 1 s, in eighths of a cell.
pub fn bar(ns: u64, cells: usize) -> String {
    let eighths = (log2_eighths(ns).clamp(BAR_FROM, BAR_TO) - BAR_FROM) as usize * cells * 8 / (BAR_TO - BAR_FROM) as usize;
    let eighths = if ns > 0 { eighths.max(1) } else { 0 };
    fill(eighths, cells)
}

/// A bar `cells` wide, `part` of `whole` long (linear).
pub fn share(part: u64, whole: u64, cells: usize) -> String {
    let eighths = (part as u128 * cells as u128 * 8 / whole.max(1) as u128).min(cells as u128 * 8) as usize;
    fill(eighths, cells)
}

// `eighths` of block characters, padded with spaces to `cells`.
fn fill(eighths: usize, cells: usize) -> String {
    const PARTS: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];
    let mut out = String::new();
    for _ in 0..eighths / 8 { out.push('█'); }
    if eighths % 8 != 0 { out.push(PARTS[eighths % 8]); }
    while out.chars().count() < cells { out.push(' '); }
    out
}

/// How a column's text is aligned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align { Left, Right }

/// A table's columns, drawn with box lines: each column is `width` characters with a space on either side.
#[derive(Clone, Debug)]
pub struct Table { pub columns: Vec<(usize, Align)> }

impl Table {
    pub fn new(columns: &[(usize, Align)]) -> Self { Self { columns: columns.to_vec() } }

    /// The total width in columns.
    pub fn width(&self) -> usize { 1 + self.columns.iter().map(|(w, _)| w + 3).sum::<usize>() }

    pub fn top(&self) -> String { self.rule('┌', '┬', '┐') }
    pub fn middle(&self) -> String { self.rule('├', '┼', '┤') }
    pub fn bottom(&self) -> String { self.rule('└', '┴', '┘') }

    fn rule(&self, left: char, cross: char, right: char) -> String {
        let mut out = String::new();
        out.push(left);
        for (i, (width, _)) in self.columns.iter().enumerate() {
            if i > 0 { out.push(cross); }
            for _ in 0..width + 2 { out.push('─'); }
        }
        out.push(right);
        out
    }

    /// A row of cells, each cut to its column with `…` where too long; missing cells are blank.
    pub fn row(&self, cells: &[&str]) -> String {
        let mut out = String::from("│");
        for (i, &(width, align)) in self.columns.iter().enumerate() {
            let text = fit(cells.get(i).copied().unwrap_or(""), width);
            let pad = width - text.chars().count();
            out.push(' ');
            if align == Align::Right { for _ in 0..pad { out.push(' '); } }
            out.push_str(&text);
            if align == Align::Left { for _ in 0..pad { out.push(' '); } }
            out.push_str(" │");
        }
        out
    }
}

/// `text` cut to `width` characters, the last one `…` when it was longer.
pub fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width { return String::from(text); }
    let mut out: String = text.chars().take(width.saturating_sub(1)).collect();
    if width > 0 { out.push('…'); }
    out
}

/// The histogram's lines for the log: each bucket's range, a bar of its share and its count.
pub fn histogram_lines(stats: &Stats, format: fn(u64) -> String) -> Vec<String> {
    let most = stats.histogram.iter().map(|&(_, n)| n).max().unwrap_or(1) as u64;
    stats.histogram.iter().map(|&(k, n)| {
        let (from, to) = (bucket_floor(k), if k == 0 { 0 } else { bucket_floor(k + 1) - 1 });
        format!("    {:>9} .. {:<9} {} {}", format(from), format(to), share(n as u64, most, 30), n)
    }).collect()
}

/// A measurement's statistics for the log, in nanoseconds and readable.
pub fn stats_lines(name: &str, unit: &str, stats: &Stats) -> Vec<String> {
    let mut out = Vec::new();
    out.push(format!("{} ({} samples, {})", name, stats.count, unit));
    out.push(format!("    min {} ns  median {} ns  mean {} ns  p99 {} ns  max {} ns", stats.min, stats.median, stats.mean, stats.p99, stats.max));
    out.push(format!("    min {}  median {}  mean {}  p99 {}  max {}", duration(stats.min), duration(stats.median), duration(stats.mean), duration(stats.p99), duration(stats.max)));
    out.extend(histogram_lines(stats, duration));
    out
}

/// The verdict of one check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Passed(String),
    Failed(String),
    /// Not run, with the reason: no such device, not granted.
    Skipped(String),
}

impl Verdict {
    pub fn mark(&self) -> &'static str { match self { Verdict::Passed(_) => "✓", Verdict::Failed(_) => "✗", Verdict::Skipped(_) => "○" } }
    pub fn word(&self) -> &'static str { match self { Verdict::Passed(_) => "PASS", Verdict::Failed(_) => "FAIL", Verdict::Skipped(_) => "SKIP" } }
    pub fn detail(&self) -> &str { match self { Verdict::Passed(d) | Verdict::Failed(d) | Verdict::Skipped(d) => d } }
}

/// The counts of passed, failed and skipped checks.
pub fn tally(verdicts: &[Verdict]) -> (usize, usize, usize) {
    verdicts.iter().fold((0, 0, 0), |(p, f, s), v| match v {
        Verdict::Passed(_) => (p + 1, f, s), Verdict::Failed(_) => (p, f + 1, s), Verdict::Skipped(_) => (p, f, s + 1),
    })
}

/// The highest NNNN of the names `<prefix>NNNN<suffix>`, 0 for none.
pub fn highest_number<'a>(names: impl Iterator<Item = &'a str>, prefix: &str, suffix: &str) -> u32 {
    names.filter_map(|name| name.strip_prefix(prefix)?.strip_suffix(suffix)?.parse::<u32>().ok()).max().unwrap_or(0)
}
