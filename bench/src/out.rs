//! The screen and the log of `check`, `bench` and `kbench`: lines go to the console, to the log or to both, and the log
//! is written to `log:<tool>NNNN.txt` (this boot's number) or, without the log volume, to `ram:<tool>-NNN.txt`.
use crate::report;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{CAP_KIND_ENDPOINT, SLOT_FILE};
use mind::idl::sysinfo;

/// The full log, kept in memory until `save`: what the screen showed, then the details.
pub struct Log { tool: &'static str, text: String, details: String, started: u64 }

impl Log {
    /// Files go through the client the launcher lent for the user's files (REQUEST_FILES), which may write on `log:` and `ram:`.
    pub fn new(tool: &'static str) -> Self {
        if mind::dev::cap_info(SLOT_FILE).0 == CAP_KIND_ENDPOINT { mind::fs::use_endpoint(mind::ipc::Endpoint(SLOT_FILE)); }
        Self { tool, text: String::new(), details: String::new(), started: mind::time::monotonic_ns() }
    }

    /// A line on the screen and in the log.
    pub fn both(&mut self, line: &str) { mind::println!("{}", line); self.log(line); }

    /// A line in the log only.
    pub fn log(&mut self, line: &str) { self.text.push_str(line); self.text.push('\n'); }

    /// A line in the log's details, after what the screen showed.
    pub fn detail(&mut self, line: &str) { self.details.push_str(line); self.details.push('\n'); }

    /// Lines in the log's details.
    pub fn detail_all(&mut self, lines: &[String]) { for line in lines { self.detail(line); } }

    /// Nanoseconds since the tool started.
    pub fn elapsed(&self) -> u64 { mind::time::monotonic_ns() - self.started }

    /// Writes the log; its path, or why it could not be written.
    pub fn save(&self) -> Result<String, String> {
        let mut tried = Vec::new();
        let text = format!("{}\nDetails\n\n{}", self.text, self.details);
        for path in candidates(self.tool) {
            match write(&path, text.as_bytes()) {
                Ok(()) => return Ok(path),
                Err(error) => tried.push(format!("{}: {:?}", path, error)),
            }
        }
        Err(tried.join("; "))
    }
}

// Where the log may go, in order: log:<tool>NNNN.txt, then -2 … -9, then ram:<tool>-NNN.txt.
fn candidates(tool: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut names: Vec<String> = Vec::new();
    if mind::fs::list("log:", |entry| names.push(String::from_utf8_lossy(entry.name).to_lowercase())).is_ok() {
        let boot = report::highest_number(names.iter().map(String::as_str), "boot", ".log");
        let free = |name: &String| !names.iter().any(|n| n == &name[4..]);
        let first = format!("log:{}{:04}.txt", tool, boot);
        if free(&first) { out.push(first); }
        out.extend((2..10).map(|k| format!("log:{}{:04}-{}.txt", tool, boot, k)).filter(free).take(1));
    }
    out.extend((1..1000).map(|n| format!("ram:{}-{:03}.txt", tool, n)).find(|name| mind::fs::metadata(name).is_err()));
    out
}

fn write(path: &str, data: &[u8]) -> Result<(), mind::fs::Error> {
    let mut file = mind::fs::File::create(path)?;
    for chunk in data.chunks(16384) { file.write(chunk)?; }
    file.flush()
}

/// The machine, for the head of the screen and the log: the architecture, the processors and the kernel arena (from
/// sysmon, REQUEST_SYSINFO), the clock, the date.
pub fn machine(info: &mind::BootInfo) -> Vec<String> {
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
    let sysmon = mind::ipc::Endpoint::SYSINFO;
    let cpus = match sysinfo::cpus(sysmon, 0) {
        Ok(Ok(list)) => format!("{} of {} processors online", list.as_slice().iter().filter(|c| c.online).count(), list.len()),
        _ => String::from("processors unknown (no sysmon client)"),
    };
    let arena = match sysinfo::memory(sysmon) {
        Ok(Ok(m)) => format!("kernel arena {} of {} used, {} tasks, {} endpoints", report::bytes(m.used), report::bytes(m.arena), m.tasks, m.endpoints),
        _ => String::from("kernel arena unknown"),
    };
    let (now, resolution, tsc) = mind::time::clock_info();
    let counter = if cfg!(target_arch = "aarch64") { "generic timer" } else { "TSC" };
    let clock = if tsc != 0 { format!("{} {}.{:03} MHz, resolution {} ns", counter, tsc / 1_000_000, tsc / 1000 % 1000, resolution) } else { format!("the tick, resolution {}", report::duration(resolution)) };
    let date = match (mind::rtc::date(), mind::rtc::seconds_since_midnight()) {
        (Some((y, m, d)), Some(s)) => format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, s / 3600, s / 60 % 60, s % 60),
        _ => String::from("date unknown"),
    };
    alloc::vec![
        format!("MIND Core, {}, ABI {}, {}, screen {}x{}", arch, info.abi_version, cpus, info.width, info.height),
        format!("{}; clock: {}", arena, clock),
        format!("{}, up {}", date, report::duration(now)),
    ]
}
