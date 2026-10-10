//! The measuring tools' run (`kbench`, `bench`): a table whose rows are a measurement's median with a bar on a log
//! scale, its minimum and its 99th percentile; notes under it; the log gets every statistic with a histogram.
use crate::out::{self, Log};
use crate::report::{self, Align, Stats, Table};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

pub fn now() -> u64 { mind::time::monotonic_ns() }

/// `samples` samples, each the mean of `batch` runs of `op`, in ns.
pub fn batches(samples: usize, batch: usize, mut op: impl FnMut()) -> Vec<u64> {
    (0..samples).map(|_| { let t = now(); for _ in 0..batch { op(); } (now() - t) / batch as u64 }).collect()
}

pub struct Run { pub log: Log, table: Table, pub quick: bool, pub rows: usize, pub failed: usize, notes: Vec<String> }

impl Run {
    /// Prints the title, the machine and the table's head.
    pub fn start(tool: &'static str, title: &str, info: &mind::BootInfo, args: &str, quick: bool) -> Self {
        let table = Table::new(&[(24, Align::Left), (8, Align::Right), (15, Align::Left), (8, Align::Right), (8, Align::Right)]);
        let mut run = Self { log: Log::new(tool), table, quick, rows: 0, failed: 0, notes: Vec::new() };
        run.log.both(title);
        for line in out::machine(info) { run.log.both(&line); }
        run.log.both(if quick { "--quick: a tenth of the repetitions" } else { "full repetitions (--quick: a tenth)" });
        run.log.log(&format!("arguments: {}", args));
        let lines = [run.table.top(), run.table.row(&["measurement", "median", "8 ns  log  1 s", "min", "p99"]), run.table.middle()];
        for line in lines { run.log.both(&line); }
        run
    }

    /// The repetitions of a measurement: a tenth with --quick, at least 3.
    pub fn reps(&self, full: usize) -> usize { if self.quick { (full / 10).max(3) } else { full } }

    pub fn group(&mut self, title: &str) {
        let line = self.table.row(&[title]);
        self.log.both(&line);
        self.log.detail(&format!("== {} (at {})", title, report::duration(self.log.elapsed())));
    }

    /// A measured row: `samples` in ns, each the mean of `batch` operations.
    pub fn row(&mut self, name: &str, samples: &mut [u64], batch: usize) -> Stats {
        let stats = Stats::of(samples);
        self.rows += 1;
        let line = self.table.row(&[&format!("  {}", name), &report::duration(stats.median), &report::bar(stats.median, 15), &report::duration(stats.min), &report::duration(stats.p99)]);
        self.log.both(&line);
        let unit = if batch > 1 { format!("ns, each sample the mean of {} operations", batch) } else { String::from("ns") };
        self.log.detail_all(&report::stats_lines(name, &unit, &stats));
        stats
    }

    /// A measurement that could not be made, with the reason.
    pub fn fail(&mut self, name: &str, why: &str) {
        self.rows += 1;
        self.failed += 1;
        let line = self.table.row(&[&format!("  {}", name), "failed", why]);
        self.log.both(&line);
        self.log.detail(&format!("{}: FAILED: {}", name, why));
    }

    /// A measurement left out, with the reason (no such device, not granted).
    pub fn skip(&mut self, name: &str, why: &str) {
        let line = self.table.row(&[&format!("  {}", name), "skipped", why]);
        self.log.both(&line);
        self.log.detail(&format!("{}: SKIPPED: {}", name, why));
    }

    /// A line under the table.
    pub fn note(&mut self, text: String) { self.notes.push(text); }

    /// Closes the table, prints the notes and the summary, writes the log; exits with 1 if a measurement failed.
    pub fn finish(mut self, tool: &str) {
        let bottom = self.table.bottom();
        self.log.both(&bottom);
        for note in core::mem::take(&mut self.notes) { self.log.both(&format!("  • {}", note)); }
        let summary = format!("{}: {} measurements, {} failed, {}", tool, self.rows, self.failed, report::duration(self.log.elapsed()));
        self.log.log(&summary);
        match self.log.save() {
            Ok(path) => mind::println!("{}; full log: {}", summary, path),
            Err(why) => mind::println!("{}; the log was not written ({})", summary, why),
        }
        if self.failed > 0 { mind::process::exit_with(1); }
    }
}
