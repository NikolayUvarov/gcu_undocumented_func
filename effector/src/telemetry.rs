//! Telemetry for the Effector server (750-EFF-0014): a schema 1 sample of `POST /api/telemetry`, as the server's other
//! agents send it every 30 s (Effector's docs/monitoring.md), from two of the gateway's counter samples
//! (`idl/effector.wit` 1.1) and the session's own view of the network cards. Rates need two samples, so the first
//! carries none, as the other agents' first does; a missing value is left out, never sent as zero. Counters only: no
//! file, log or screen data.
use crate::json::Writer;
use alloc::string::String;
use alloc::vec::Vec;

pub const PERIOD_MS: u64 = 30_000;
/// The longest body: 48 services, 4 volumes and 4 cards fit well within it; the server takes 64 KiB.
pub const BODY_MAX: usize = 24 * 1024;

/// A boot service and its process: `state` is the server's word (`running`, `stopped`, `quarantined`, `not_found`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Load { pub name: String, pub state: &'static str, pub pid: u64, pub run_ns: u64, pub started_ns: u64, pub memory: u64 }

/// A volume: its name (empty for the boot volume), size and free space in bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Space { pub name: String, pub bytes: u64, pub free: u64 }

/// One sample of the gateway's counters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    pub cpus: u16, pub busy_ns: u64, pub idle_ns: u64, pub uptime_ms: u64, pub memory_total: u64, pub memory_free: u64,
    pub services: Vec<Load>, pub volumes: Vec<Space>,
}

/// A network card as the session's network grant sees it: its number, kind, link, MAC and IPv4 address (0: none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Card { pub card: u8, pub wireless: bool, pub up: bool, pub mac: u64, pub address: u32 }

/// What names the sample and what the session counts itself.
#[derive(Clone, Copy, Debug)]
pub struct Identity<'a> {
    pub sample_id: &'a str, pub sampled_at: &'a str, pub cpu_model: &'a str, pub arch: &'a str,
    pub executed: u64, pub executed_total: u64,
}

/// `part` of `whole` in thousandths of a percent, at most 100%.
fn percent(part: u128, whole: u128) -> Option<u64> {
    (whole > 0).then(|| (part.min(whole) * 100_000 / whole) as u64)
}

/// The sample's JSON, rates taken against `before` when it is the same boot's earlier sample.
pub fn sample<'b>(buf: &'b mut [u8], now: &Counters, before: Option<&Counters>, cards: &[Card], id: &Identity<'_>) -> Result<&'b [u8], crate::json::Error> {
    let elapsed_ms = before.map(|b| now.uptime_ms.saturating_sub(b.uptime_ms)).filter(|&ms| ms > 0 && ms <= 3_600_000);
    let before = before.filter(|_| elapsed_ms.is_some());
    let mut w = Writer::new(buf);
    w.object()?;
    w.field_u64("schema", 1)?;
    w.field_str("sample_id", id.sample_id)?;
    w.field_str("sampled_at", id.sampled_at)?;
    w.field_decimal("interval_seconds", elapsed_ms.unwrap_or(PERIOD_MS))?;

    w.key("system")?;
    w.object()?;
    w.field_str("cpu_model", id.cpu_model)?;
    w.field_str("architecture", id.arch)?;
    w.field_u64("logical_cpus", now.cpus as u64)?;
    if let Some(b) = before {
        let (busy, idle) = (now.busy_ns.saturating_sub(b.busy_ns) as u128, now.idle_ns.saturating_sub(b.idle_ns) as u128);
        if let Some(p) = percent(busy, busy + idle) { w.field_decimal("cpu_percent", p)?; }
    }
    w.field_u64("uptime_seconds", now.uptime_ms / 1000)?;
    w.field_u64("executed_requests", id.executed)?;
    w.field_u64("executed_requests_total", id.executed_total)?;
    if now.memory_total > 0 {
        w.field_u64("memory_total", now.memory_total)?;
        w.field_u64("memory_available", now.memory_free.min(now.memory_total))?;
        w.field_u64("memory_used", now.memory_total - now.memory_free.min(now.memory_total))?;
    }
    w.end()?;

    w.key("disks")?;
    w.array()?;
    for v in now.volumes.iter() {
        w.object()?;
        w.field_str("name", if v.name.is_empty() { "boot" } else { &v.name })?;
        w.field_str("kind", "volume")?;
        let mount = match v.name.as_str() { "" => String::from("/"), n => { let mut m = String::from(n); m.push(':'); m } };
        w.field_str("mount", &mount)?;
        w.field_u64("total_bytes", v.bytes)?;
        w.field_u64("free_bytes", v.free.min(v.bytes))?;
        w.end()?;
    }
    w.end()?;

    w.key("interfaces")?;
    w.array()?;
    for c in cards {
        w.object()?;
        let mut text = String::new();
        let _ = core::fmt::write(&mut text, format_args!("card{}", c.card));
        w.field_str("name", &text)?;
        w.field_str("type", if c.wireless { "wireless" } else { "ethernet" })?;
        w.field_str("state", if c.up { "up" } else { "down" })?;
        if c.mac != 0 {
            text.clear();
            for (i, b) in c.mac.to_be_bytes()[2..].iter().enumerate() { let _ = core::fmt::write(&mut text, format_args!("{}{:02x}", if i == 0 { "" } else { ":" }, b)); }
            w.field_str("mac_address", &text)?;
        }
        if c.address != 0 {
            text.clear();
            let a = c.address.to_be_bytes();
            let _ = core::fmt::write(&mut text, format_args!("{}.{}.{}.{}", a[0], a[1], a[2], a[3]));
            w.key("ip_addresses")?; w.array()?; w.string(&text)?; w.end()?;
        }
        w.end()?;
    }
    w.end()?;

    w.key("components")?;
    w.array()?;
    for s in now.services.iter() {
        w.object()?;
        w.field_str("id", &s.name)?;
        w.field_str("name", &s.name)?;
        w.field_str("program", &s.name)?;
        w.field_str("state", s.state)?;
        w.key("pids")?; w.array()?; if s.pid != 0 { w.unsigned(s.pid)?; } w.end()?;
        if s.pid != 0 {
            // The same process in both samples: its PID and its start time, so a restarted one is not compared.
            let earlier = before.and_then(|b| b.services.iter().find(|e| e.name == s.name && e.pid == s.pid && e.started_ns == s.started_ns));
            if let (Some(e), Some(ms)) = (earlier, elapsed_ms) {
                let wall = ms as u128 * 1_000_000 * now.cpus.max(1) as u128;
                if let Some(p) = percent(s.run_ns.saturating_sub(e.run_ns) as u128, wall) { w.field_decimal("cpu_percent", p)?; }
            }
            if s.memory > 0 { w.field_u64("working_set_bytes", s.memory)?; }
        }
        w.end()?;
    }
    w.end()?;
    w.end()?;
    w.finish()
}
