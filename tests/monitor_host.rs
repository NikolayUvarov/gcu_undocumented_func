//! Host tests of the system monitors (monitor/src): CPU use from deltas, sorting, the spawn tree, the physical map,
//! graphs and the hardware report, against a fake sysmon; every tool is drawn on small and large grids.
#![allow(dead_code)]
extern crate alloc;
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/keys.rs"]
mod keys;
#[path = "../libmind/src/util.rs"]
mod util;
#[path = "../libmind/src/tui/mod.rs"]
mod tui;
#[path = "../monitor/src/model.rs"]
mod model;
#[path = "../monitor/src/text.rs"]
mod text;
#[path = "../monitor/src/top.rs"]
mod top;
#[path = "../monitor/src/memmap.rs"]
mod memmap;
#[path = "../monitor/src/load.rs"]
mod load;
#[path = "../monitor/src/hw.rs"]
mod hw;

use abi::*;
use keys::{event, Key};
use model::*;
use tui::{Cell, Grid, DARK};

const MS: u64 = 1_000_000;

fn task(pid: u64, parent: u64, name: &str, service: bool) -> Task {
    Task { pid, parent, name: name.into(), state: TASK_RECV, flags: if service { TASK_FLAG_SERVICE } else { 0 }, caps: 3, quota_endpoints: 4, ..Task::default() }
}

#[derive(Default)]
struct Fake { tasks: Vec<Task>, now: u64, ranges: Vec<Range>, samples: Vec<Sample>, slow_requested: Vec<(bool, u16)>, vmap_requests: Vec<u64>, devices: Vec<Device>, irqs: Vec<Irq>,
              lifecycle: bool, stopped: Vec<u64>, restarted: Vec<String> }

impl Source for Fake {
    fn tasks(&mut self) -> Result<Vec<Task>, Problem> { Ok(self.tasks.clone()) }
    fn cpus(&mut self) -> Result<Vec<Cpu>, Problem> { Ok((0..4).map(|i| Cpu { apic: i, online: i < 2, ..Cpu::default() }).collect()) }
    fn memory(&mut self) -> Result<Memory, Problem> {
        Ok(Memory { arena: 64 << 20, used: 16 << 20, free: 48 << 20, largest_free: 40 << 20, images: 1 << 20, screens: 8 << 20, heaps: 4 << 20, objects_limit: 16 << 20, dma_limit: 8 << 20,
                    tasks: self.tasks.len() as u32, tasks_limit: 24, endpoints: 8, endpoints_limit: 63, ..Memory::default() })
    }
    fn physmap(&mut self) -> Result<Vec<Range>, Problem> { Ok(self.ranges.clone()) }
    fn vmap(&mut self, pid: u64) -> Result<Vec<Region>, Problem> {
        self.vmap_requests.push(pid);
        Ok(vec![Region { start: 0x80_0000_0000, bytes: 8192, kind: VM_CODE, flags: VM_READ | VM_EXEC }, Region { start: 0x80_0100_0000, bytes: 4096, kind: VM_GUARD, flags: 0 },
                Region { start: 0x80_0100_1000, bytes: 65536, kind: VM_STACK, flags: VM_READ | VM_WRITE }])
    }
    fn caps(&mut self, _pid: u64) -> Result<Vec<Capability>, Problem> {
        Ok(vec![Capability { slot: 2, kind: CAP_KIND_ENDPOINT as u32, ..Capability::default() }, Capability { slot: 12, kind: CAP_KIND_MEMORY as u32, ..Capability::default() }])
    }
    fn irqs(&mut self) -> Result<Vec<Irq>, Problem> { Ok(self.irqs.clone()) }
    fn devices(&mut self) -> Result<Vec<Device>, Problem> { Ok(self.devices.clone()) }
    fn history(&mut self, slow: bool, count: u16) -> Result<Vec<Sample>, Problem> {
        self.slow_requested.push((slow, count));
        Ok(self.samples.iter().rev().take(count as usize).rev().copied().collect())
    }
    fn load(&mut self) -> Result<Load, Problem> { Ok(Load { one: 12, five: 8, fifteen: 1, uptime_ms: 3_723_000, fast_ms: 100, slow_ms: 1000, ..Load::default() }) }
    fn now_ns(&self) -> u64 { self.now }
    fn stop(&mut self, task: &Task) -> Result<(), String> {
        if !self.lifecycle { return Err(String::from(NO_LIFECYCLE)); }
        if task.name == "init" { return Err(String::from("init and the shell cannot be stopped")); }
        self.stopped.push(task.pid);
        self.tasks.retain(|t| t.pid != task.pid);
        Ok(())
    }
    fn restart(&mut self, name: &str) -> Result<u64, String> {
        if !self.lifecycle { return Err(String::from(NO_LIFECYCLE)); }
        self.restarted.push(name.into());
        Ok(99)
    }
}

fn system() -> Fake {
    let mut tasks = vec![task(1, 0, "init", true), task(5, 1, "loader", true), task(7, 1, "shell", true), task(12, 5, "busy", false), task(13, 5, "top", false)];
    for t in &mut tasks { t.started_ns = 0; }
    Fake { tasks, now: 1000 * MS, ..Fake::default() }
}

fn chr(ch: char) -> Key { Key(event(0, ch as u32, 0)) }
fn code(code: u32) -> Key { Key(event(code, 0, 0)) }

fn draw(tool: &mut dyn Tool, cols: usize, rows: usize) -> Vec<String> {
    let mut cells = vec![Cell::BLANK; cols * rows];
    let mut grid = Grid::new(&mut cells, cols, rows);
    tool.draw(&mut grid, &DARK);
    (0..rows).map(|y| (0..cols).map(|x| grid.get(x, y).ch).collect()).collect()
}

#[test]
fn numbers() {
    assert_eq!(text::size(0), "0B");
    assert_eq!(text::size(1536), "1.5K");
    assert_eq!(text::size(64 << 20), "64.0M");
    assert_eq!(text::size(1500 << 20), "1.4G");
    assert_eq!(text::size(355 << 20), "355M");
    assert_eq!(text::count(1_234_567), "1 234 567");
    assert_eq!(text::count(999), "999");
    assert_eq!(text::cpu_time(83_456 * MS), "1:23.45");
    assert_eq!(text::cpu_time(3_723_000 * MS), "1:02:03");
    assert_eq!(text::uptime(3_723_000), "1:02:03");
    assert_eq!(text::uptime(90_061_000), "1d 1:01:01");
    assert_eq!(text::hundredths(7), "0.07");
    assert_eq!(text::permille(425), "42.5");
    assert_eq!([0, 3, 11, 2360, 54_880].map(text::nice_max), [1, 5, 20, 5000, 100_000]);
    assert_eq!(text::rights(VM_READ | VM_EXEC), "r-x");
}

#[test]
fn cpu_use_comes_from_run_time_deltas() {
    let mut source = system();
    let mut top = top::Top::new();
    top.refresh(&mut source).unwrap();
    // First sight: measured since start (busy ran 600 ms of its first second).
    source.tasks[3].run_ns = 0;
    source.tasks[3].run_ns = 600 * MS;
    source.tasks[3].calls = 10;
    top.update(source.tasks.clone(), source.now);
    // One second later busy ran 500 ms more and made 100 syscalls; a newcomer ran 100 ms of its 200.
    source.now += 1000 * MS;
    source.tasks[3].run_ns += 500 * MS;
    source.tasks[3].calls += 100;
    let mut fresh = task(14, 5, "new", false);
    fresh.started_ns = source.now - 200 * MS;
    fresh.run_ns = 100 * MS;
    source.tasks.push(fresh);
    top.update(source.tasks.clone(), source.now);
    assert_eq!(top.usage_of(12).cpu, 500);
    assert_eq!(top.usage_of(12).syscalls, 100);
    assert_eq!(top.usage_of(14).cpu, 500);
    assert_eq!(top.usage_of(1).cpu, 0);
    // A counter that went backwards (a reused PID) is measured since start, never negative.
    source.tasks[3].run_ns = 10 * MS;
    source.now += 1000 * MS;
    top.update(source.tasks.clone(), source.now);
    assert!(top.usage_of(12).cpu <= 1000);
}

#[test]
fn per_cpu_load_and_rates_from_samples() {
    let mut source = system();
    source.samples = (0..10).map(|i| Sample { busy: [1000, 200, 0, 0, 0, 0, 0, 0], messages: 3, syscalls: 50 + i, interrupts: 10, switches: 20, ..Sample::default() }).collect();
    let mut top = top::Top::new();
    top.refresh(&mut source).unwrap();
    assert_eq!(source.slow_requested.last(), Some(&(false, 10)), "a 1 s interval reads ten 100 ms samples");
    assert_eq!(top.busy, vec![1000, 200], "two CPUs online");
    assert_eq!(top.rates.messages, 30);
    assert_eq!(top.rates.syscalls, 545);
    assert_eq!(top.rates.interrupts, 100);
}

#[test]
fn sorting_filter_and_tree_keep_the_selection() {
    let mut source = system();
    let mut top = top::Top::new();
    source.tasks[2].run_ns = 300 * MS; // shell
    source.tasks[3].run_ns = 900 * MS; // busy
    source.tasks[4].heap = 8 << 20; // top
    top.refresh(&mut source).unwrap();
    let pids = |top: &top::Top| top.rows().iter().map(|r| r.0.pid).collect::<Vec<_>>();
    assert_eq!(pids(&top), [12, 7, 1, 5, 13], "by CPU, then PID");
    assert_eq!(top.selected_pid(), Some(12));
    let _ = draw(&mut top, 100, 30);
    assert_eq!(top.key(chr('M'), &mut source), Flow::Redraw);
    assert_eq!(pids(&top)[0], 13, "by memory");
    assert_eq!(top.selected_pid(), Some(12), "the selection stays on the task");
    assert_eq!(top.rows()[top.list.selected].0.pid, 12);
    top.key(chr('N'), &mut source);
    assert_eq!(pids(&top), [1, 5, 7, 12, 13]);
    top.key(chr('T'), &mut source);
    assert_eq!(pids(&top)[0], 12, "by CPU time");
    // Russian layout: the key of S hides services.
    top.key(chr('ы'), &mut source);
    assert_eq!(pids(&top), [12, 13]);
    top.key(chr('s'), &mut source);
    top.key(chr('N'), &mut source);
    top.key(chr('t'), &mut source);
    let tree: Vec<(u64, usize)> = top.rows().iter().map(|r| (r.0.pid, r.1)).collect();
    assert_eq!(tree, [(1, 0), (5, 1), (12, 2), (13, 2), (7, 1)]);
    let screen = draw(&mut top, 100, 30);
    assert!(screen.iter().any(|l| l.contains("  └ busy")), "{:#?}", screen);
    assert!(top.status().contains("TREE=1"), "{}", top.status());
}

#[test]
fn details_window_and_keys() {
    let mut source = system();
    let mut top = top::Top::new();
    top.refresh(&mut source).unwrap();
    let _ = draw(&mut top, 100, 30);
    top.key(chr('N'), &mut source);
    top.key(code(KEY_DOWN), &mut source);
    assert_eq!(top.selected_pid(), Some(5));
    assert_eq!(top.key(code(KEY_ENTER), &mut source), Flow::Redraw);
    assert_eq!(source.vmap_requests, [5]);
    let details = top.details.as_ref().unwrap();
    let lines = top::Top::details_lines(details, source.now);
    assert!(lines[0].contains("PID 5  loader  service"), "{:?}", lines);
    assert!(lines[1].contains("Parent 1 init"), "{:?}", lines);
    assert!(lines.iter().any(|l| l == "Capabilities 3/31: endpoint 1, memory 1"), "{:?}", lines);
    assert!(lines.iter().any(|l| l.contains("3 regions, 72.0K mapped, 1 guard pages")), "{:?}", lines);
    let screen = draw(&mut top, 100, 30);
    assert!(screen.iter().any(|l| l.contains("Task 5")), "{:#?}", screen);
    assert_eq!(top.key(chr('x'), &mut source), Flow::Ignored, "other keys wait while the window is open");
    assert_eq!(top.key(code(KEY_ESC), &mut source), Flow::Redraw);
    assert!(top.details.is_none());
    top.key(chr('k'), &mut source);
    assert!(top.status().ends_with("CONFIRM=STOP"));
    top.key(code(KEY_ESC), &mut source);
    assert_eq!(top.key(chr('+'), &mut source), Flow::Redraw);
    assert_eq!(top.interval_ms(), 2000);
    top.key(chr('-'), &mut source);
    top.key(chr('-'), &mut source);
    top.key(chr('-'), &mut source);
    assert_eq!(top.interval_ms(), 500);
    assert_eq!(top.key(chr('q'), &mut source), Flow::Quit);
    assert_eq!(top.key(code(KEY_ESC), &mut source), Flow::Quit);
}

#[test]
fn stop_and_restart_through_init() {
    let mut source = system();
    source.lifecycle = true;
    let mut top = top::Top::new();
    top.refresh(&mut source).unwrap();
    let _ = draw(&mut top, 100, 30);
    top.key(chr('N'), &mut source);
    top.key(code(KEY_END), &mut source);
    top.key(code(KEY_UP), &mut source);
    assert_eq!(top.selected_pid(), Some(12));
    // r is for services; k asks first, and Cancel is the default.
    top.key(chr('r'), &mut source);
    assert!(top.notice.as_deref().unwrap().starts_with("Only boot services restart"));
    top.key(chr('k'), &mut source);
    assert!(top.status().ends_with("CONFIRM=STOP"), "{}", top.status());
    let screen = draw(&mut top, 100, 30);
    assert!(screen.iter().any(|l| l.contains("Stop busy (PID 12)?")) && screen.iter().any(|l| l.contains("[ Cancel ]")), "{:#?}", screen);
    top.key(code(KEY_ENTER), &mut source);
    assert!(source.stopped.is_empty() && top.confirm.is_none());
    top.key(chr('k'), &mut source);
    top.key(code(KEY_LEFT), &mut source);
    assert_eq!(top.key(code(KEY_ENTER), &mut source), Flow::Refresh);
    assert_eq!(source.stopped, [12]);
    assert_eq!(top.notice.as_deref(), Some("busy (PID 12) stopped"));
    top.refresh(&mut source).unwrap();
    assert!(top.tasks.iter().all(|t| t.pid != 12));
    // A service restarts.
    top.key(code(KEY_HOME), &mut source);
    top.key(code(KEY_DOWN), &mut source);
    assert_eq!(top.selected_pid(), Some(5));
    top.key(chr('r'), &mut source);
    assert!(top.status().ends_with("CONFIRM=RESTART"));
    let screen = draw(&mut top, 100, 30);
    assert!(screen.iter().any(|l| l.contains("Restart loader (PID 5)?")), "{:#?}", screen);
    top.key(code(KEY_LEFT), &mut source);
    top.key(code(KEY_ENTER), &mut source);
    assert_eq!(source.restarted, ["loader"]);
    assert_eq!(top.notice.as_deref(), Some("loader restarted as PID 99"));
    // init refuses; without a lifecycle client the reason is shown.
    top.key(code(KEY_HOME), &mut source);
    top.key(chr('k'), &mut source);
    top.key(code(KEY_LEFT), &mut source);
    top.key(code(KEY_ENTER), &mut source);
    assert_eq!(top.notice.as_deref(), Some("Stop init: init and the shell cannot be stopped"));
    source.lifecycle = false;
    top.key(code(KEY_DOWN), &mut source);
    top.key(chr('k'), &mut source);
    top.key(code(KEY_LEFT), &mut source);
    top.key(code(KEY_ENTER), &mut source);
    assert!(top.notice.as_deref().unwrap().ends_with(NO_LIFECYCLE), "{:?}", top.notice);
}

#[test]
fn columns_fit_the_screen() {
    for width in [40, 60, 79, 100, 159] {
        let columns = top::Top::columns(width);
        let used: usize = columns.iter().map(|c| c.1 + 1).sum();
        assert!(used <= width || columns.len() <= 5, "{} {:?}", width, columns);
        for name in ["PID", "NAME", "%CPU"] { assert!(columns.iter().any(|c| c.0 == name), "{} lacks {}", width, name); }
    }
    assert_eq!(top::Top::columns(159).len(), 13);
}

fn physmap() -> Vec<Range> {
    vec![Range { start: 0, bytes: 0x1000, kind: 3, detail: 3 }, Range { start: 0x1000, bytes: 0x9F000, kind: 7, detail: 7 }, Range { start: 0x100000, bytes: 0x700000, kind: 7, detail: 7 },
         Range { start: 0x800000, bytes: 0x800000, kind: 7, detail: 7 }, Range { start: 0x1000000, bytes: 0x400_0000, kind: 2, detail: 2 },
         Range { start: 0x1000000, bytes: 0x400_0000, kind: PHYS_HEAP, detail: 0 }, Range { start: 0x8000_0000, bytes: 0x40_0000, kind: PHYS_FRAMEBUFFER, detail: 0 },
         Range { start: 0xFD_0000_0000, bytes: 0x3_0000_0000, kind: 0, detail: 0 }]
}

#[test]
fn physical_map() {
    let ranges = physmap();
    let merged = memmap::merge(&ranges);
    // The two touching free ranges become one; the arena follows the loader data it lies in.
    assert_eq!(merged.iter().filter(|r| r.kind == 7).count(), 2);
    assert!(merged.iter().any(|r| r.kind == 7 && r.start == 0x100000 && r.bytes == 0xF00000));
    let arena = merged.iter().position(|r| r.kind == PHYS_HEAP).unwrap();
    assert_eq!(merged[arena - 1].kind, 2);
    // 4 GiB in 64 cells of 64 MiB: the first is mostly free RAM, the arena wins over loader data, then a hole, the framebuffer.
    let bar = memmap::bar_kinds(&ranges, 4 << 30, 64);
    assert_eq!(bar[0], Some(PHYS_HEAP));
    assert_eq!(bar[1], Some(PHYS_HEAP), "the arena ends at 80 MiB");
    assert_eq!(bar[2], None);
    assert_eq!(bar[32], Some(PHYS_FRAMEBUFFER));
    let bar = memmap::bar_kinds(&ranges, 0x1000000, 16);
    assert_eq!(bar[0], Some(7));
    assert_eq!(memmap::ram(&ranges), (0x9F000 + 0xF00000 + 0x400_0000 + 0x1000, 0x9F000 + 0xF00000));
}

#[test]
fn memmap_views() {
    let mut source = system();
    source.ranges = physmap();
    let mut map = memmap::Memmap::new();
    map.refresh(&mut source).unwrap();
    let screen = draw(&mut map, 100, 30);
    assert!(screen.iter().any(|l| l.contains("RAM 79.6M usable, 15.6M free; 8 ranges")), "{:#?}", screen);
    assert!(screen.iter().any(|l| l.contains("kernel arena")));
    assert_eq!(map.key(chr('m'), &mut source), Flow::Redraw);
    assert!(!map.merged);
    assert_eq!(map.key(chr('2'), &mut source), Flow::Redraw);
    let screen = draw(&mut map, 100, 30);
    assert!(screen.iter().any(|l| l.contains("Kernel arena 64.0M: used 16.0M (25.0%)")), "{:#?}", screen);
    assert!(screen.iter().any(|l| l.contains("screens") && l.contains("8.0M") && l.contains("12.5%")), "{:#?}", screen);
    map.key(code(KEY_TAB), &mut source);
    assert_eq!(map.view, memmap::View::Process);
    assert_eq!(source.vmap_requests, [1]);
    let _ = draw(&mut map, 100, 30);
    map.key(code(KEY_END), &mut source);
    assert_eq!(source.vmap_requests, [1, 13]);
    let screen = draw(&mut map, 100, 30);
    assert!(screen.iter().any(|l| l.contains("Address space of top (PID 13)")), "{:#?}", screen);
    assert!(screen.iter().any(|l| l.contains("0x0000008001001000     64.0K  rw-    stack")), "{:#?}", screen);
    assert!(map.status().starts_with("VIEW=PROCESS PID=13"), "{}", map.status());
    map.key(chr('4'), &mut source);
    let screen = draw(&mut map, 100, 30);
    assert!(screen.iter().any(|l| l.contains("     busy")), "the quota tree indents children: {:#?}", screen);
    assert_eq!(map.key(chr('q'), &mut source), Flow::Quit);
}

#[test]
fn graphs() {
    assert_eq!(load::resample(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 5), [1, 3, 5, 7, 9]);
    assert_eq!(load::resample(&[4, 4], 10), [4, 4]);
    assert_eq!(load::resample(&[1, 2, 3], 2), [1, 2]);
    let mut source = system();
    source.samples = (0..300).map(|i| Sample { busy: [500, 1000, 0, 0, 0, 0, 0, 0], interrupts: 7, used_kib: 16384, tasks: 5 + (i % 2) as u8, ..Sample::default() }).collect();
    let mut view = load::LoadView::new();
    view.refresh(&mut source).unwrap();
    assert_eq!(source.slow_requested, [(false, 300)]);
    assert_eq!(view.values(load::Series::Interrupts)[0], 70, "per second from 100 ms samples");
    assert_eq!(view.values(load::Series::CpuTotal)[0], 750);
    assert_eq!(view.series().len(), 8, "two CPUs online, six counters");
    let title = view.title(load::Series::Arena, &view.values(load::Series::Arena));
    assert_eq!(title, "kernel arena  16.0M of 64.0M  max 16.0M");
    let screen = draw(&mut view, 160, 50);
    assert!(screen.iter().any(|l| l.contains("CPU1  100.0%")), "{:#?}", screen);
    assert!(screen.iter().any(|l| l.contains('⣿')), "graphs drawn in braille");
    view.key(chr('c'), &mut source);
    assert_eq!(view.series()[0], load::Series::CpuTotal);
    view.key(chr('2'), &mut source);
    assert_eq!(source.slow_requested.last(), Some(&(true, 600)));
    assert!(view.status().starts_with("WINDOW=10MIN TOTAL=1"));
}

#[test]
fn hardware_report() {
    let mut source = system();
    source.ranges = physmap();
    source.devices = vec![Device { class: 0x010180, location: 0x000109, holder: 5, irq: 14, bars: [0, 0, 0, 0, 16, 0], io_bars: 1 << 4, ..Device::default() },
                          Device { class: 0x0C0330, location: 0x000400, holder: 0, irq: 11, bars: [0x4000, 0, 0, 0, 0, 0], ..Device::default() }];
    source.irqs = vec![Irq { line: 1, count: 1234, holder: 7, endpoint: 4, masked: false }, Irq { line: 3, ..Irq::default() }];
    let local = hw::Local { vendor: "GenuineIntel".into(), brand: "Test CPU".into(), family: 6, model: 85, stepping: 4, features: vec![("NX", true), ("x2APIC", false)],
                            tsc_hz: 2_400_000_000, resolution_ns: 1, width: 1280, height: 800, stride: 1280 };
    let mut report = hw::Hw::new(local);
    report.refresh(&mut source).unwrap();
    let lines: Vec<String> = report.lines().into_iter().map(|l| l.0).collect();
    let has = |text: &str| assert!(lines.iter().any(|l| l.contains(text)), "{:?} lacks {:?}", lines, text);
    has("Test CPU");
    has("GenuineIntel, family 6, model 85, stepping 4");
    has("2 CPUs online, APIC IDs 0, 1");
    has("+NX  -x2APIC");
    has("TSC 2400.000 MHz");
    has("GOP framebuffer 1280x800, 1280 pixels per line, 32 bits per pixel, 4.0M at 0x80000000");
    has("00:01.1  010180  IDE controller   IRQ 14  loader (PID 5)");
    has("BAR4 16B io");
    has("USB xHCI");
    has("IRQ 1         1 234 interrupts  shell (PID 7), endpoint 4");
    assert!(!lines.iter().any(|l| l.contains("IRQ 3 ")), "unused lines are left out");
    let _ = draw(&mut report, 80, 10);
    report.key(code(KEY_TAB), &mut source);
    assert!(report.top > 0);
    assert_eq!(report.key(chr('r'), &mut source), Flow::Refresh);
    assert_eq!(report.key(code(KEY_ESC), &mut source), Flow::Quit);
}

#[test]
fn every_tool_draws_on_any_screen() {
    let mut source = system();
    source.ranges = physmap();
    source.samples = vec![Sample::default(); 50];
    let mut tools: Vec<Box<dyn Tool>> = vec![Box::new(top::Top::new()), Box::new(memmap::Memmap::new()), Box::new(load::LoadView::new()), Box::new(hw::Hw::new(hw::Local::default()))];
    for tool in tools.iter_mut() {
        tool.refresh(&mut source).unwrap();
        for (cols, rows) in [(20, 6), (40, 12), (80, 25), (100, 37), (160, 50), (240, 67)] {
            for key in ['1', '2', '3', '4', 'c'] {
                let _ = tool.key(chr(key), &mut source);
                let _ = draw(tool.as_mut(), cols, rows);
                for code_ in [KEY_DOWN, KEY_END, KEY_PGDN, KEY_TAB] { let _ = tool.key(code(code_), &mut source); let _ = draw(tool.as_mut(), cols, rows); }
            }
        }
    }
}
