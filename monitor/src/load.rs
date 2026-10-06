//! `load`: graphs of CPU busy time, interrupts, syscalls, IPC, context switches, kernel memory and tasks over the last
//! 30 s (100 ms samples) or 10 min (1 s samples), with the load averages (docs/tools §4.6).
use crate::keys::{Code, Key};
use crate::model::*;
use crate::text;
use crate::tui::{Grid, Rect, Style, Theme};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// What a graph shows; values per sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Series { Cpu(usize), CpuTotal, Interrupts, Syscalls, Messages, Switches, Arena, Tasks }

/// `values` squeezed into `points` points by averaging neighbours (or as they are, if they fit).
pub fn resample(values: &[u64], points: usize) -> Vec<u64> {
    if values.len() <= points || points == 0 { return values.to_vec(); }
    (0..points).map(|i| {
        let (a, b) = (values.len() * i / points, (values.len() * (i + 1) / points).max(values.len() * i / points + 1));
        values[a..b].iter().sum::<u64>() / (b - a) as u64
    }).collect()
}

pub struct LoadView {
    pub samples: Vec<Sample>,
    pub slow: bool,  // 10 min of 1 s samples instead of 30 s of 100 ms samples
    pub total: bool, // one CPU graph for all CPUs
    pub load: Load,
    pub memory: Memory,
    online: usize,
}

impl Default for LoadView { fn default() -> Self { Self::new() } }

impl LoadView {
    pub fn new() -> Self { Self { samples: Vec::new(), slow: false, total: false, load: Load::default(), memory: Memory::default(), online: 0 } }

    fn period_ms(&self) -> u64 { let p = if self.slow { self.load.slow_ms } else { self.load.fast_ms }; if p == 0 { if self.slow { 1000 } else { 100 } } else { p as u64 } }

    pub fn series(&self) -> Vec<Series> {
        let mut list: Vec<Series> = if self.total { alloc::vec![Series::CpuTotal] } else { (0..self.online.clamp(1, 8)).map(Series::Cpu).collect() };
        list.extend([Series::Interrupts, Series::Syscalls, Series::Messages, Series::Switches, Series::Arena, Series::Tasks]);
        list
    }

    /// The values of a series, one per sample; rates per second.
    pub fn values(&self, series: Series) -> Vec<u64> {
        let per_second = |count: u32| count as u64 * 1000 / self.period_ms();
        let online = self.online.clamp(1, 8);
        self.samples.iter().map(|s| match series {
            Series::Cpu(i) => s.busy[i] as u64,
            Series::CpuTotal => s.busy[..online].iter().map(|&b| b as u64).sum::<u64>() / online as u64,
            Series::Interrupts => per_second(s.interrupts), Series::Syscalls => per_second(s.syscalls),
            Series::Messages => per_second(s.messages), Series::Switches => per_second(s.switches),
            Series::Arena => s.used_kib as u64 * 1024, Series::Tasks => s.tasks as u64,
        }).collect()
    }

    /// The fixed top of a series' scale, or none (scaled to its maximum).
    fn limit(&self, series: Series) -> Option<u64> {
        match series { Series::Cpu(_) | Series::CpuTotal => Some(1000), Series::Arena => Some(self.memory.arena.max(1)), Series::Tasks => Some(self.memory.tasks_limit.max(1) as u64), _ => None }
    }

    /// Title of a graph: the current, average and highest value.
    pub fn title(&self, series: Series, values: &[u64]) -> String {
        let now = values.last().copied().unwrap_or(0);
        let avg = if values.is_empty() { 0 } else { values.iter().sum::<u64>() / values.len() as u64 };
        let max = values.iter().copied().max().unwrap_or(0);
        match series {
            Series::Cpu(_) | Series::CpuTotal => {
                let name = if let Series::Cpu(i) = series { format!("CPU{}", i) } else { format!("CPU total ({})", self.online) };
                format!("{}  {}%  avg {}%  max {}%", name, text::permille(now as u32), text::permille(avg as u32), text::permille(max as u32))
            }
            Series::Arena => format!("kernel arena  {} of {}  max {}", text::size(now), text::size(self.memory.arena), text::size(max)),
            Series::Tasks => format!("tasks  {} of {}  max {}", now, self.memory.tasks_limit, max),
            _ => {
                let name = match series { Series::Interrupts => "interrupts", Series::Syscalls => "syscalls", Series::Messages => "IPC messages", _ => "context switches" };
                format!("{}  {}/s  avg {}/s  max {}/s", name, text::count(now), text::count(avg), text::count(max))
            }
        }
    }

    fn color(series: Series) -> u32 {
        match series { Series::Cpu(_) | Series::CpuTotal => 0xA6E3A1, Series::Arena => 0xF080C0, Series::Tasks => 0xE0E060, _ => 0x80D0FF }
    }

    /// The top of a series' graph and its label (`100%`, `50 000/s`, `64.0M`, `32`).
    fn scale(&self, series: Series, values: &[u64]) -> (u64, String) {
        let max = self.limit(series).unwrap_or_else(|| text::nice_max(values.iter().copied().max().unwrap_or(0).max(1)));
        let label = match series { Series::Cpu(_) | Series::CpuTotal => String::from("100%"), Series::Arena => text::size(max), Series::Tasks => format!("{}", max), _ => format!("{}/s", text::count(max)) };
        (max, label)
    }

    // A graph in `area`: its title, then the plot; the `reserve` columns at the right, the same for every graph (the
    // widest label and a space, issue u012), hold its label right-aligned, so all plots end at one column.
    fn graph(&self, grid: &mut Grid, area: Rect, series: Series, reserve: usize, theme: &Theme) {
        if area.h < 2 || area.w < 8 { return; }
        let values = self.values(series);
        let (max, scale) = self.scale(series, &values);
        let plot = Rect::new(area.x, area.y + 1, area.w.saturating_sub(reserve), area.h - 1);
        grid.text_max(area.x, area.y, &self.title(series, &values), plot.w, theme.header);
        grid.text_right(area.right(), area.y, &scale, theme.dim);
        grid.fill(plot, ' ', theme.panel);
        grid.graph(plot, &resample(&values, plot.w * 2), max, Style::new(Self::color(series), theme.panel.bg));
        grid.vline(plot.right(), plot.y, plot.h, crate::tui::Line::Single, theme.dim);
    }
}

impl Tool for LoadView {
    fn refresh(&mut self, source: &mut dyn Source) -> Result<(), Problem> {
        if self.online == 0 { self.online = source.cpus()?.iter().filter(|c| c.online).count().max(1); }
        self.memory = source.memory()?;
        self.load = source.load()?;
        self.samples = source.history(self.slow, if self.slow { 600 } else { 300 })?;
        Ok(())
    }

    fn draw(&mut self, grid: &mut Grid, theme: &Theme) {
        let (w, h) = (grid.cols, grid.rows);
        grid.clear(theme.panel);
        grid.fill(Rect::new(0, 0, w, 1), ' ', theme.status);
        let window = if self.slow { "10 min, 1 s samples" } else { "30 s, 100 ms samples" };
        grid.text(1, 0, &format!("load   {}   load average {} {} {}   up {}", window, text::hundredths(self.load.one), text::hundredths(self.load.five),
                                  text::hundredths(self.load.fifteen), text::uptime(self.load.uptime_ms)), theme.status);
        let series = self.series();
        let reserve = series.iter().map(|&s| self.scale(s, &self.values(s)).1.chars().count()).max().unwrap_or(0) + 1;
        let rows = h.saturating_sub(2);
        // One column, or two on a wide screen when one is too short for every graph to get a title and two rows.
        let columns = if w >= 100 && series.len() * 3 > rows { 2 } else { 1 };
        let per_column = series.len().div_ceil(columns);
        let height = (rows / per_column.max(1)).max(2);
        let width = (w - 1) / columns;
        for (i, &s) in series.iter().enumerate() {
            let (column, index) = (i / per_column, i % per_column);
            let y = 1 + index * height;
            if y + 2 > h - 1 { continue; } // no room: the graph is left out
            let area = Rect::new(1 + column * width, y, width.saturating_sub(2), height.min(h - 1 - y).saturating_sub(if height > 3 { 1 } else { 0 }));
            self.graph(grid, area, s, reserve, theme);
        }
        grid.fill(Rect::new(0, h - 1, w, 1), ' ', theme.status);
        grid.text(1, h - 1, "1 30 s  2 10 min  c per CPU/total  q quit", theme.status);
    }

    fn key(&mut self, key: Key, source: &mut dyn Source) -> Flow {
        if matches!(key.code(), Code::Esc | Code::F(10)) { return Flow::Quit; }
        match key.latin() {
            Some('q') | Some('Q') => Flow::Quit,
            Some('1') => { self.slow = false; let _ = self.refresh(source); Flow::Redraw }
            Some('2') => { self.slow = true; let _ = self.refresh(source); Flow::Redraw }
            Some('c') | Some('C') => { self.total = !self.total; Flow::Redraw }
            _ => Flow::Ignored,
        }
    }

    fn interval_ms(&self) -> u64 { 1000 }

    fn status(&self) -> String { format!("WINDOW={} TOTAL={} SAMPLES={}", if self.slow { "10MIN" } else { "30S" }, self.total as u8, self.samples.len()) }
}
