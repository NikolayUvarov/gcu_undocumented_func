//! `hw`: processor, clocks, display, PCI devices with the services holding them, interrupt lines and the memory the
//! platform uses (docs/tools §4.7). What the program learns itself (CPUID, clock, screen) comes in `Local`.
use crate::abi::*;
use crate::keys::{Code, Key};
use crate::model::*;
use crate::text;
use crate::tui::{Grid, Rect, Style, Theme};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// What the program reads itself: CPUID, the clock and its screen.
#[derive(Clone, Debug, Default)]
pub struct Local {
    pub vendor: String, pub brand: String, pub family: u32, pub model: u32, pub stepping: u32,
    pub features: Vec<(&'static str, bool)>,
    pub tsc_hz: u64, pub resolution_ns: u64,
    pub width: usize, pub height: usize, pub stride: usize,
}

#[cfg(target_arch = "x86_64")]
impl Local {
    /// CPUID leaves 0, 1, 7, 0x80000001..4 and 0x80000007.
    #[allow(unused_unsafe)]
    pub fn cpuid() -> Self {
        use core::arch::x86_64::__cpuid;
        let id = |leaf: u32| unsafe { __cpuid(leaf) };
        let word = |value: u32| value.to_le_bytes();
        let zero = id(0);
        let mut vendor = String::new();
        for b in [word(zero.ebx), word(zero.edx), word(zero.ecx)].concat() { if b != 0 { vendor.push(b as char); } }
        let one = if zero.eax >= 1 { id(1) } else { id(0) };
        let seven = if zero.eax >= 7 { unsafe { core::arch::x86_64::__cpuid_count(7, 0) } } else { core::arch::x86_64::CpuidResult { eax: 0, ebx: 0, ecx: 0, edx: 0 } };
        let max_ext = id(0x8000_0000).eax;
        let ext = if max_ext >= 0x8000_0001 { id(0x8000_0001) } else { core::arch::x86_64::CpuidResult { eax: 0, ebx: 0, ecx: 0, edx: 0 } };
        let power = if max_ext >= 0x8000_0007 { id(0x8000_0007) } else { core::arch::x86_64::CpuidResult { eax: 0, ebx: 0, ecx: 0, edx: 0 } };
        let mut brand = String::new();
        if max_ext >= 0x8000_0004 {
            for leaf in 0x8000_0002..=0x8000_0004u32 { let r = id(leaf); for b in [word(r.eax), word(r.ebx), word(r.ecx), word(r.edx)].concat() { if b != 0 { brand.push(b as char); } } }
        }
        let base_family = (one.eax >> 8) & 0xF;
        let family = if base_family == 0xF { base_family + ((one.eax >> 20) & 0xFF) } else { base_family };
        let model = if base_family == 0x6 || base_family == 0xF { ((one.eax >> 4) & 0xF) | ((one.eax >> 12) & 0xF0) } else { (one.eax >> 4) & 0xF };
        let features = alloc::vec![
            ("NX", ext.edx & (1 << 20) != 0), ("invariant TSC", power.edx & (1 << 8) != 0), ("xAPIC", one.edx & (1 << 9) != 0), ("x2APIC", one.ecx & (1 << 21) != 0),
            ("SSE2", one.edx & (1 << 26) != 0), ("SSE4.2", one.ecx & (1 << 20) != 0), ("AVX", one.ecx & (1 << 28) != 0), ("AVX2", seven.ebx & (1 << 5) != 0),
            ("RDRAND", one.ecx & (1 << 30) != 0), ("SMEP", seven.ebx & (1 << 7) != 0), ("SMAP", seven.ebx & (1 << 20) != 0), ("1 GiB pages", ext.edx & (1 << 26) != 0),
        ];
        Self { vendor, brand: String::from(brand.trim()), family, model, stepping: one.eax & 0xF, features, ..Self::default() }
    }
}

/// A line of the report: text and how to show it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Heading, Text, Dim }

pub struct Hw {
    pub local: Local,
    pub cpus: Vec<Cpu>,
    pub devices: Vec<Device>,
    pub irqs: Vec<Irq>,
    pub ranges: Vec<Range>,
    pub tasks: Vec<Task>,
    pub memory: Memory,
    pub uptime_ms: u64,
    pub top: usize,
    height: usize,
}

impl Hw {
    pub fn new(local: Local) -> Self {
        Self { local, cpus: Vec::new(), devices: Vec::new(), irqs: Vec::new(), ranges: Vec::new(), tasks: Vec::new(), memory: Memory::default(), uptime_ms: 0, top: 0, height: 10 }
    }

    fn holder(&self, pid: u32) -> String {
        if pid == 0 { String::from("—") } else { format!("{} (PID {})", task_name(&self.tasks, pid as u64), pid) }
    }

    /// The report, line by line.
    pub fn lines(&self) -> Vec<(String, Kind)> {
        let mut out: Vec<(String, Kind)> = Vec::new();
        let mut line = |text: String, kind: Kind| out.push((text, kind));
        let l = &self.local;
        line(String::from("Processor"), Kind::Heading);
        if !l.brand.is_empty() { line(format!("  {}", l.brand), Kind::Text); }
        line(format!("  {}, family {}, model {}, stepping {}", l.vendor, l.family, l.model, l.stepping), Kind::Text);
        let online: Vec<String> = self.cpus.iter().filter(|c| c.online).map(|c| format!("{}", c.apic)).collect();
        line(format!("  {} CPUs online, APIC IDs {}", online.len(), online.join(", ")), Kind::Text);
        let features: Vec<String> = l.features.iter().map(|&(name, on)| format!("{}{}", if on { '+' } else { '-' }, name)).collect();
        line(format!("  {}", features.join("  ")), Kind::Text);
        line(String::new(), Kind::Text);
        line(String::from("Clocks"), Kind::Heading);
        if l.tsc_hz != 0 {
            line(format!("  TSC {}.{:03} MHz (calibrated), monotonic clock resolution {} ns", l.tsc_hz / 1_000_000, l.tsc_hz / 1000 % 1000, l.resolution_ns), Kind::Text);
        } else {
            line(format!("  10 ms timer tick (the TSC rate is not constant), resolution {} ns", l.resolution_ns), Kind::Text);
        }
        line(format!("  up {}", text::uptime(self.uptime_ms)), Kind::Text);
        line(String::new(), Kind::Text);
        line(String::from("Display"), Kind::Heading);
        let frame = self.ranges.iter().find(|r| r.kind == PHYS_FRAMEBUFFER);
        line(format!("  GOP framebuffer {}x{}, {} pixels per line, 32 bits per pixel{}", l.width, l.height, l.stride,
                     frame.map_or(String::new(), |r| format!(", {} at {:#x}", text::size(r.bytes), r.start))), Kind::Text);
        line(String::new(), Kind::Text);
        line(format!("PCI devices ({})", self.devices.len()), Kind::Heading);
        for d in &self.devices {
            let mut bars = String::new();
            for (i, &bytes) in d.bars.iter().enumerate().filter(|(_, b)| **b != 0) {
                bars.push_str(&format!(" BAR{} {}{}", i, text::size(bytes), if d.io_bars & (1 << i) != 0 { " io" } else { "" }));
            }
            let irq = if d.irq == 0 || d.irq == 0xFF { String::from("  —   ") } else { format!("IRQ {:<2}", d.irq) };
            line(format!("  {:02x}:{:02x}.{}  {:06X}  {:<16} {}  {:<22}{}", d.location >> 16, (d.location >> 8) & 0xFF, d.location & 7, d.class, text::pci_class(d.class),
                         irq, self.holder(d.holder), bars), Kind::Text);
        }
        line(String::new(), Kind::Text);
        line(String::from("Interrupt lines"), Kind::Heading);
        let mut any = false;
        for i in self.irqs.iter().filter(|i| i.holder != 0 || i.count != 0) {
            any = true;
            line(format!("  IRQ {:<2} {:>12} interrupts  {}{}{}", i.line, text::count(i.count), self.holder(i.holder),
                         if i.endpoint != 0 { format!(", endpoint {}", i.endpoint) } else { String::new() }, if i.masked { ", masked" } else { "" }), Kind::Text);
        }
        if !any { line(String::from("  none in use"), Kind::Dim); }
        line(String::new(), Kind::Text);
        line(String::from("Memory"), Kind::Heading);
        let usable: u64 = self.ranges.iter().filter(|r| matches!(r.kind, 1..=4 | 7)).map(|r| r.bytes).sum();
        let free: u64 = self.ranges.iter().filter(|r| r.kind == 7).map(|r| r.bytes).sum();
        line(format!("  RAM {} usable, {} free after boot (firmware map)", text::size(usable), text::size(free)), Kind::Text);
        let sum = |kind: u32| self.ranges.iter().filter(|r| r.kind == kind).map(|r| r.bytes).sum::<u64>();
        let images = self.ranges.iter().filter(|r| r.kind == PHYS_BOOT_IMAGE).count();
        line(format!("  kernel {}, kernel arena {}, {} boot images {}", text::size(sum(PHYS_KERNEL)), text::size(sum(PHYS_HEAP)), images, text::size(sum(PHYS_BOOT_IMAGE))), Kind::Text);
        line(format!("  DMA buffers {} of {}, device registers {}", text::size(self.memory.dma), text::size(self.memory.dma_limit), text::size(sum(PHYS_DEVICE))), Kind::Text);
        line(String::new(), Kind::Text);
        line(String::from("  Block devices and the audio codec are not listed yet: their drivers do not report them."), Kind::Dim);
        out
    }

    fn sections(lines: &[(String, Kind)]) -> Vec<usize> { lines.iter().enumerate().filter(|(_, l)| l.1 == Kind::Heading).map(|(i, _)| i).collect() }
}

impl Tool for Hw {
    fn refresh(&mut self, source: &mut dyn Source) -> Result<(), Problem> {
        self.cpus = source.cpus()?;
        self.tasks = source.tasks()?;
        self.devices = source.devices()?;
        self.irqs = source.irqs()?;
        self.ranges = source.physmap()?;
        self.memory = source.memory()?;
        self.uptime_ms = source.now_ns() / 1_000_000;
        Ok(())
    }

    fn draw(&mut self, grid: &mut Grid, theme: &Theme) {
        let (w, h) = (grid.cols, grid.rows);
        grid.clear(theme.panel);
        grid.fill(Rect::new(0, 0, w, 1), ' ', theme.status);
        grid.text(1, 0, "hw   hardware and its holders", theme.status);
        let lines = self.lines();
        self.height = h.saturating_sub(2);
        self.top = self.top.min(lines.len().saturating_sub(self.height));
        for (i, (text, kind)) in lines.iter().enumerate().skip(self.top).take(self.height) {
            let style = match kind { Kind::Heading => theme.header, Kind::Text => theme.panel, Kind::Dim => theme.dim };
            grid.text_max(1, 1 + i - self.top, text, w - 2, style);
        }
        if lines.len() > self.height {
            grid.text_right(w, 1, &format!("{}/{}", (self.top + self.height).min(lines.len()), lines.len()), Style::new(theme.dim.fg, theme.panel.bg));
        }
        grid.fill(Rect::new(0, h - 1, w, 1), ' ', theme.status);
        grid.text(1, h - 1, "↑↓ PgUp PgDn scroll  Tab next section  r refresh  q quit", theme.status);
    }

    fn key(&mut self, key: Key, _source: &mut dyn Source) -> Flow {
        let len = self.lines().len();
        let last = len.saturating_sub(self.height);
        let page = self.height.max(2) - 1;
        self.top = match key.code() {
            Code::Esc | Code::F(10) => return Flow::Quit,
            Code::Up => self.top.saturating_sub(1), Code::Down => (self.top + 1).min(last),
            Code::PageUp => self.top.saturating_sub(page), Code::PageDown => (self.top + page).min(last),
            Code::Home => 0, Code::End => last,
            Code::Tab => Self::sections(&self.lines()).into_iter().find(|&s| s > self.top).unwrap_or(0).min(last),
            _ => match key.latin() { Some('q') | Some('Q') => return Flow::Quit, Some('r') | Some('R') => return Flow::Refresh, _ => return Flow::Ignored },
        };
        Flow::Redraw
    }

    fn interval_ms(&self) -> u64 { 5000 }

    fn status(&self) -> String { format!("TOP={} LINES={}", self.top, self.lines().len()) }
}
