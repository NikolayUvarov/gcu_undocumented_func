//! The cable shared by the machines on it (800-LNK-0026): one port, and a worker for each machine's address, each with
//! its own client. An exchange takes the line for itself, in the order the workers asked, so the machines are asked
//! one at a time and none waits for another's whole transfer, only for the exchange under way.
use crate::port::Port;
use linkproto::Line;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub struct Bus {
    spec: String,
    baud: u32,
    /// The port, opened at the first exchange and again after it failed; why it last would not open.
    port: Mutex<(Option<Port>, String)>,
    /// The turns given and the one served: the line goes to the workers in the order they asked.
    turn: Mutex<(u64, u64)>,
    next: Condvar,
    start: Instant,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> { m.lock().unwrap_or_else(|e| e.into_inner()) }

impl Bus {
    pub fn new(spec: &str, baud: u32) -> Arc<Bus> {
        Arc::new(Bus { spec: spec.into(), baud, port: Mutex::new((None, String::new())), turn: Mutex::new((0, 0)), next: Condvar::new(), start: Instant::now() })
    }

    /// A serial port, rather than TCP: its wait for an answer comes from its rate.
    pub fn serial(&self) -> bool { !self.spec.contains(':') }

    pub fn spec(&self) -> &str { &self.spec }

    /// Why the port would not open at the last try; empty while it is open.
    pub fn error(&self) -> String { lock(&self.port).1.clone() }

    pub fn line(self: &Arc<Self>) -> BusLine { BusLine { bus: self.clone(), held: false } }

    /// The port opened when it is not, or failed.
    fn ready(&self) {
        let mut p = lock(&self.port);
        if p.0.as_ref().is_some_and(|port| !port.failed()) { return; }
        p.0 = None;
        match Port::open(&self.spec, self.baud) {
            Ok(mut port) => {
                port.turnaround = Duration::ZERO;
                if let Some(l) = port.latency_ms().filter(|&l| self.serial() && l > 2) {
                    eprintln!("effector485: {}: the FTDI latency timer is {l} ms, which slows every answer (mindlink hello says how to lower it)", port.name);
                }
                *p = (Some(port), String::new());
            }
            Err(e) => p.1 = e,
        }
    }
}

/// One worker's way onto the shared port.
pub struct BusLine { bus: Arc<Bus>, held: bool }

impl BusLine {
    /// The port is not open, or failed for good.
    pub fn failed(&self) -> bool { lock(&self.bus.port).0.as_ref().is_none_or(|p| p.failed()) }
}

impl Line for BusLine {
    fn write(&mut self, bytes: &[u8]) -> bool { lock(&self.bus.port).0.as_mut().is_some_and(|p| p.write(bytes)) }

    fn read(&mut self, out: &mut [u8], timeout_ms: u32) -> usize {
        let mut p = lock(&self.bus.port);
        match p.0.as_mut() {
            Some(port) => port.read(out, timeout_ms),
            None => { drop(p); std::thread::sleep(Duration::from_millis(timeout_ms.min(100) as u64)); 0 }
        }
    }

    fn now_ms(&mut self) -> u64 { self.bus.start.elapsed().as_millis() as u64 }

    fn claim(&mut self) {
        let mut t = lock(&self.bus.turn);
        let mine = t.0;
        t.0 += 1;
        while t.1 != mine { t = self.bus.next.wait(t).unwrap_or_else(|e| e.into_inner()); }
        drop(t);
        self.held = true;
        self.bus.ready();
    }

    fn release(&mut self) {
        if !std::mem::take(&mut self.held) { return; }
        lock(&self.bus.turn).1 += 1;
        self.bus.next.notify_all();
    }
}

/// A worker that panics in an exchange gives its turn back, so the other machines are still served.
impl Drop for BusLine {
    fn drop(&mut self) { self.release(); }
}
