//! The link protocol over a simulated line that loses, corrupts, echoes and repeats bytes (800-LNK-0001).
use linkproto::app::{path_name, plain_name, Client, FileService, Files, RunOutput, EXIT_UNKNOWN, INPUT_MAX, MIN_PAYLOAD, RUN_ENDED, RUN_RUNNING, RUN_STOPPED};
use linkproto::endpoint::{channel, code, kind};
use linkproto::frame::{self, Decoder, Header, MAX_PAYLOAD, MAX_WIRE};
use linkproto::sha256::Sha256;
use linkproto::{cobs, crc, Error, Line, Service, Slave};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

/// xorshift64*: the same faults on every run.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 >> 12; self.0 ^= self.0 << 25; self.0 ^= self.0 >> 27; self.0.wrapping_mul(0x2545F4914F6CDD1D) }
    fn chance(&mut self, per_million: u64) -> bool { self.next() % 1_000_000 < per_million }
}

/// What the line does to bytes, in parts per million: a whole write lost, a byte lost, a byte changed, a write sent twice.
#[derive(Clone, Copy, Default)]
struct Faults { lose_write: u64, lose_byte: u64, flip_byte: u64, repeat_write: u64, echo: bool }

struct Wire { rng: Rng, faults: Faults }
impl Wire {
    fn carry(&mut self, bytes: &[u8], to: &mut VecDeque<u8>) {
        if self.rng.chance(self.faults.lose_write) { return; }
        let times = if self.rng.chance(self.faults.repeat_write) { 2 } else { 1 };
        for _ in 0..times {
            for &b in bytes {
                if self.rng.chance(self.faults.lose_byte) { continue; }
                to.push_back(if self.rng.chance(self.faults.flip_byte) { b ^ (1 << (self.rng.next() % 8)) } else { b });
            }
        }
    }
}

/// Both directions and the clock, shared by the two ends.
struct Shared { to_slave: VecDeque<u8>, to_master: VecDeque<u8>, wire: Wire, clock: u64, turnarounds: u32, held_until: u64 }
type Link = Rc<RefCell<Shared>>;

struct SlaveEnd(Link);
impl Line for SlaveEnd {
    fn write(&mut self, bytes: &[u8]) -> bool {
        let s = &mut *self.0.borrow_mut();
        s.wire.carry(bytes, &mut s.to_master);
        if s.wire.faults.echo { s.to_slave.extend(bytes); }
        true
    }
    fn read(&mut self, out: &mut [u8], _timeout_ms: u32) -> usize { take(&mut self.0.borrow_mut().to_slave, out) }
    fn now_ms(&mut self) -> u64 { self.0.borrow().clock }
    fn turnaround(&mut self) { self.0.borrow_mut().turnarounds += 1; }
}

/// The master's end runs the slave whenever it waits, as the other machine would meanwhile.
struct MasterEnd { link: Link, slave: Slave<SlaveEnd>, service: FileService<Disk> }
impl Line for MasterEnd {
    fn write(&mut self, bytes: &[u8]) -> bool {
        let s = &mut *self.link.borrow_mut();
        s.wire.carry(bytes, &mut s.to_slave);
        if s.wire.faults.echo { s.to_master.extend(bytes); }
        true
    }
    fn read(&mut self, out: &mut [u8], timeout_ms: u32) -> usize {
        while !self.link.borrow().to_slave.is_empty() { self.slave.serve(&mut self.service, 0).unwrap(); }
        // A slave busy writing its disk answers only once the write is done.
        let stall = core::mem::take(&mut self.service.files.stalled);
        if stall > 0 { let s = &mut *self.link.borrow_mut(); s.held_until = s.clock + stall; }
        if self.link.borrow().clock < self.link.borrow().held_until { self.link.borrow_mut().clock += timeout_ms as u64; return 0; }
        let n = take(&mut self.link.borrow_mut().to_master, out);
        // Nothing came: the wait ran its course.
        self.link.borrow_mut().clock += if n == 0 { timeout_ms as u64 } else { 1 };
        n
    }
    fn now_ms(&mut self) -> u64 { self.link.borrow().clock }
}

fn take(q: &mut VecDeque<u8>, out: &mut [u8]) -> usize {
    let n = q.len().min(out.len());
    for b in out[..n].iter_mut() { *b = q.pop_front().unwrap(); }
    n
}

/// Files in memory, counting writes: a repeated request must not write twice.
#[derive(Default)]
struct Disk { files: BTreeMap<String, Vec<u8>>, dirs: BTreeSet<String>, open: Option<(String, Vec<u8>)>, reading: Vec<u8>, writes: u32, log: Vec<u8>,
              console: Vec<u8>, runs: Vec<Run>, starts: u32, reboots: u32, commands: u32, replaced: Vec<(String, String)>, inputs: Vec<u64>,
              commit_stall: u64, stalled: u64 }

/// A program of the simulated end: `echo.elf` prints its arguments and ends; `loop.elf` prints a line a poll until stopped.
/// A shell command (COMMAND) ends at once: its line echoed with exit 0, or `refused` with exit 1 for `netpolicy add`.
struct Run { name: String, output: Vec<u8>, dropped: u64, state: u8, exit: u32 }
const KEPT: usize = 100;

impl Disk {
    fn parent_exists(&self, path: &str) -> bool { path.rsplit_once('/').map_or(true, |(dir, _)| self.dirs.contains(dir)) }
    fn print(run: &mut Run, text: &[u8]) {
        run.output.extend_from_slice(text);
        // Only the last KEPT bytes are kept, as a slave with a small buffer would.
        let over = run.output.len().saturating_sub(KEPT);
        run.output.drain(..over);
        run.dropped += over as u64;
    }
}
impl Files for Disk {
    fn create(&mut self, name: &str, size: u64) -> Result<(), u8> {
        if size > 64 << 20 { return Err(code::IO); }
        if !self.parent_exists(name) { return Err(code::NOT_FOUND); }
        self.open = Some((name.into(), Vec::new()));
        Ok(())
    }
    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), u8> {
        let (_, buf) = self.open.as_mut().ok_or(code::BAD_REQUEST)?;
        assert_eq!(offset as usize, buf.len(), "written out of order");
        buf.extend_from_slice(data);
        self.writes += 1;
        Ok(())
    }
    fn commit(&mut self, _digest: &[u8; 32], matches: bool) -> Result<(), u8> {
        // A disk slow to flush: the answer waits `commit_stall` ms.
        self.stalled = self.commit_stall;
        let (name, buf) = self.open.take().ok_or(code::BAD_REQUEST)?;
        if matches { self.files.insert(name, buf); }
        Ok(())
    }
    fn open(&mut self, name: &str) -> Result<u64, u8> {
        self.reading = self.files.get(name).ok_or(code::NOT_FOUND)?.clone();
        Ok(self.reading.len() as u64)
    }
    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> Result<usize, u8> {
        let rest = self.reading.get(offset as usize..).ok_or(code::BAD_REQUEST)?;
        let n = rest.len().min(out.len());
        out[..n].copy_from_slice(&rest[..n]);
        Ok(n)
    }
    fn stat(&mut self, name: &str) -> Result<(u64, [u8; 32]), u8> {
        let data = self.files.get(name).ok_or(code::NOT_FOUND)?;
        Ok((data.len() as u64, sha(data)))
    }
    fn delete(&mut self, name: &str) -> Result<(), u8> { self.files.remove(name).map(|_| ()).ok_or(code::NOT_FOUND) }
    fn list(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> {
        let mut used = 0;
        for (i, (name, data)) in self.files.iter().enumerate().skip(cursor as usize) {
            let line = format!("{name}\t{}\n", data.len());
            if used + line.len() > out.len() { return Ok((used, Some(i as u32))); }
            out[used..used + line.len()].copy_from_slice(line.as_bytes());
            used += line.len();
        }
        Ok((used, None))
    }
    fn log(&mut self, cursor: u64, out: &mut [u8]) -> Result<(usize, u64), u8> {
        let rest = self.log.get(cursor as usize..).unwrap_or(&[]);
        let n = rest.len().min(out.len());
        out[..n].copy_from_slice(&rest[..n]);
        Ok((n, cursor + n as u64))
    }
    fn console(&mut self, input: &[u8], out: &mut [u8]) -> Result<usize, u8> {
        self.console.extend_from_slice(input);
        let echo = input.iter().map(|b| b.to_ascii_uppercase()).collect::<Vec<_>>();
        out[..echo.len()].copy_from_slice(&echo);
        Ok(echo.len())
    }
    fn mkdir(&mut self, path: &str) -> Result<(), u8> {
        if self.files.contains_key(path) { return Err(code::EXISTS); }
        let mut at = String::new();
        for part in path.split('/') {
            if !at.is_empty() { at.push('/'); }
            at.push_str(part);
            self.dirs.insert(at.clone());
        }
        Ok(())
    }
    fn rename(&mut self, from: &str, to: &str, replace: bool) -> Result<(), u8> {
        if !self.parent_exists(to) { return Err(code::NOT_FOUND); }
        if self.files.contains_key(to) && !replace || self.dirs.contains(to) { return Err(code::EXISTS); }
        if let Some(data) = self.files.remove(from) { self.files.insert(to.into(), data); return Ok(()); }
        if !self.dirs.contains(from) { return Err(code::NOT_FOUND); }
        let under = |p: &str| p == from || p.starts_with(&format!("{from}/"));
        let moved = |p: &str| format!("{to}{}", &p[from.len()..]);
        let dirs: Vec<String> = self.dirs.iter().filter(|d| under(d)).cloned().collect();
        for d in dirs { self.dirs.remove(&d); self.dirs.insert(moved(&d)); }
        let files: Vec<String> = self.files.keys().filter(|f| under(f)).cloned().collect();
        for f in files { let data = self.files.remove(&f).unwrap(); self.files.insert(moved(&f), data); }
        Ok(())
    }
    fn rmdir(&mut self, path: &str, recursive: bool) -> Result<(), u8> {
        if !self.dirs.contains(path) { return Err(if self.files.contains_key(path) { code::DENIED } else { code::NOT_FOUND }); }
        let inside = format!("{path}/");
        let full = self.dirs.iter().any(|d| d.starts_with(&inside)) || self.files.keys().any(|f| f.starts_with(&inside));
        if full && !recursive { return Err(code::NOT_EMPTY); }
        self.dirs.retain(|d| d != path && !d.starts_with(&inside));
        self.files.retain(|f, _| !f.starts_with(&inside));
        Ok(())
    }
    fn list_dir(&mut self, path: &str, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> {
        if !path.is_empty() && !self.dirs.contains(path) { return Err(code::NOT_FOUND); }
        let prefix = if path.is_empty() { String::new() } else { format!("{path}/") };
        let direct = |p: &str| p.strip_prefix(&prefix).filter(|rest| !rest.is_empty() && !rest.contains('/')).map(str::to_string);
        let mut lines: Vec<String> = self.dirs.iter().filter_map(|d| direct(d)).map(|d| format!("{d}/\t0\n")).collect();
        lines.extend(self.files.iter().filter_map(|(f, data)| direct(f).map(|n| format!("{n}\t{}\n", data.len()))));
        let mut used = 0;
        for (i, line) in lines.iter().enumerate().skip(cursor as usize) {
            if used + line.len() > out.len() { return Ok((used, Some(i as u32))); }
            out[used..used + line.len()].copy_from_slice(line.as_bytes());
            used += line.len();
        }
        Ok((used, None))
    }
    fn run(&mut self, path: &str, args: &str) -> Result<(u32, u64), u8> {
        if !self.files.contains_key(path) { return Err(code::NOT_FOUND); }
        self.starts += 1;
        let mut run = Run { name: path.into(), output: Vec::new(), dropped: 0, state: RUN_RUNNING, exit: 0 };
        match path.rsplit('/').next().unwrap() {
            "echo.elf" => { Disk::print(&mut run, format!("{args}\n").as_bytes()); run.state = RUN_ENDED; }
            "loop.elf" => {}
            _ => return Err(code::DENIED),
        }
        self.runs.push(run);
        Ok((self.runs.len() as u32, 100 + self.runs.len() as u64))
    }
    fn output(&mut self, run: u32, cursor: u64, out: &mut [u8]) -> Result<RunOutput, u8> {
        let r = self.runs.get_mut((run as usize).wrapping_sub(1)).ok_or(code::NOT_FOUND)?;
        if r.state == RUN_RUNNING { let line = format!("tick {}\n", r.dropped + r.output.len() as u64); Disk::print(r, line.as_bytes()); }
        let end = r.dropped + r.output.len() as u64;
        let from = cursor.clamp(r.dropped, end);
        let rest = &r.output[(from - r.dropped) as usize..];
        let n = rest.len().min(out.len());
        out[..n].copy_from_slice(&rest[..n]);
        Ok(RunOutput { state: r.state, exit: if r.state == RUN_ENDED { r.exit } else { EXIT_UNKNOWN }, from, len: n })
    }
    fn stop(&mut self, run: u32) -> Result<(), u8> {
        let r = self.runs.get_mut((run as usize).wrapping_sub(1)).ok_or(code::NOT_FOUND)?;
        if r.state == RUN_RUNNING { r.state = RUN_STOPPED; }
        Ok(())
    }
    fn reboot(&mut self) -> Result<(), u8> { self.reboots += 1; Ok(()) }
    /// As MIND Core's `link` with the input capability: each word posted once, in order.
    fn input(&mut self, words: &[u64]) -> Result<u32, u8> { self.inputs.extend_from_slice(words); Ok(words.len() as u32) }
    /// As MIND Core's init: `logd` is never replaced, a file must exist; each replace is kept, a repeat too.
    fn replace(&mut self, unit: &str, path: &str) -> Result<(), u8> {
        if unit == "logd" { return Err(code::DENIED); }
        if !path.is_empty() && !self.files.contains_key(path) { return Err(code::NOT_FOUND); }
        self.replaced.push((unit.into(), path.into()));
        Ok(())
    }
    fn command(&mut self, line: &str) -> Result<u32, u8> {
        self.commands += 1;
        let word = line.split_whitespace().next().unwrap_or("");
        let mut run = Run { name: format!("sh: {word}"), output: Vec::new(), dropped: 0, state: RUN_ENDED, exit: 0 };
        if line.starts_with("netpolicy add") { Disk::print(&mut run, b"refused\n"); run.exit = 1; } else { Disk::print(&mut run, format!("$ {line}\n").as_bytes()); }
        self.runs.push(run);
        Ok(self.runs.len() as u32)
    }
    fn runs(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> {
        let mut used = 0;
        for (i, r) in self.runs.iter().enumerate().skip(cursor as usize) {
            let line = format!("{}\t{}\t{}\t{}\n", i + 1, 101 + i, ["running", "ended", "stopped"][r.state as usize], r.name);
            if used + line.len() > out.len() { return Ok((used, Some(i as u32))); }
            out[used..used + line.len()].copy_from_slice(line.as_bytes());
            used += line.len();
        }
        Ok((used, None))
    }
}

fn pair(faults: Faults, seed: u64, clock: u64) -> (Client<MasterEnd>, Link) {
    let link = Rc::new(RefCell::new(Shared { to_slave: VecDeque::new(), to_master: VecDeque::new(), wire: Wire { rng: Rng(seed), faults }, clock, turnarounds: 0, held_until: 0 }));
    let end = MasterEnd { link: link.clone(), slave: Slave::new(SlaveEnd(link.clone())), service: FileService::new(Disk::default()) };
    (Client::new(end), link)
}

fn data(len: usize, seed: u64) -> Vec<u8> {
    let mut rng = Rng(seed);
    // Zeros and runs of them, as COBS must carry.
    (0..len).map(|i| if i % 97 < 7 { 0 } else { rng.next() as u8 }).collect()
}

fn sha(bytes: &[u8]) -> [u8; 32] { let mut s = Sha256::new(); s.update(bytes); s.finish() }

fn put<L: Line>(c: &mut Client<L>, name: &str, bytes: &[u8]) -> Result<[u8; 32], Error> {
    c.put(name, bytes.len() as u64, |off, out| { let rest = &bytes[off as usize..]; let n = rest.len().min(out.len()); out[..n].copy_from_slice(&rest[..n]); n })
        .map(|(digest, _)| digest)
}

fn get<L: Line>(c: &mut Client<L>, name: &str) -> Result<Vec<u8>, Error> {
    let mut got = Vec::new();
    c.get(name, |off, chunk| { assert_eq!(off as usize, got.len()); got.extend_from_slice(chunk); true })?;
    Ok(got)
}

#[test]
fn crc32c_check_value() {
    assert_eq!(crc::crc32c(b"123456789"), 0xE306_9283);
    assert_eq!(crc::crc32c(b""), 0);
}

#[test]
fn cobs_round_trip() {
    for len in [0, 1, 2, 253, 254, 255, 256, 508, 509, 1036, frame::MAX_FRAME] {
        for fill in [0u8, 1, 0xFF] {
            let raw: Vec<u8> = (0..len).map(|i| if fill == 1 { (i % 251) as u8 } else { fill }).collect();
            let mut enc = vec![0u8; cobs::max_encoded(len)];
            let n = cobs::encode(&raw, &mut enc).unwrap();
            assert!(n <= cobs::max_encoded(len));
            assert!(!enc[..n].contains(&0), "a zero in the encoding of {len} bytes of {fill}");
            let mut dec = vec![0u8; len];
            assert_eq!(cobs::decode(&enc[..n], &mut dec), Some(len));
            assert_eq!(dec, raw);
        }
    }
    // A buffer too small is refused, not overrun.
    let mut small = [0u8; 4];
    assert_eq!(cobs::encode(&[1, 2, 3, 4, 5], &mut small), None);
}

#[test]
fn frame_round_trip_and_damage() {
    let h = Header { address: 0, channel: channel::FILES, kind: kind::PUT_DATA, flags: 0, seq: 0xBEEF };
    let payload = data(MAX_PAYLOAD, 7);
    let mut wire = [0u8; MAX_WIRE];
    let n = frame::encode(&h, &payload, &mut wire).unwrap();
    assert_eq!(wire[n - 1], 0);
    assert!(frame::encode(&h, &vec![0; MAX_PAYLOAD + 1], &mut wire.clone()).is_none());
    let mut d = Decoder::new();
    // Noise first, then the frame: the decoder finds it after the next zero.
    let got: Vec<Header> = [1u8, 2, 3, 0].iter().chain(&wire[..n]).filter_map(|&b| d.push(b)).collect();
    assert_eq!(got, vec![h]);
    assert_eq!(d.payload(), &payload[..]);
    assert_eq!(d.dropped, 1);
    // Every single changed byte is caught.
    for at in (0..n - 1).step_by(13) {
        let mut bad = wire;
        bad[at] ^= 0x10;
        if bad[at] == 0 { continue; }
        let mut d = Decoder::new();
        assert!(bad[..n].iter().all(|&b| d.push(b).is_none()), "damage at {at} not caught");
    }
}

#[test]
fn decoder_survives_noise() {
    let mut rng = Rng(0x1234_5678);
    let mut d = Decoder::new();
    for _ in 0..200_000 { let b = rng.next() as u8; let _ = d.push(if rng.chance(2_000) { 0 } else { b }); }
    // A run longer than any frame, then a good frame.
    for _ in 0..MAX_WIRE * 3 { d.push(0x55); }
    let mut wire = [0u8; MAX_WIRE];
    let h = Header { address: 0, channel: 1, kind: 2, flags: 0, seq: 3 };
    let n = frame::encode(&h, b"after the noise", &mut wire).unwrap();
    assert_eq!(wire[..n].iter().filter_map(|&b| d.push(b)).last(), None, "the overlong run ends with the frame's zero");
    let got: Vec<Header> = wire[..n].iter().filter_map(|&b| d.push(b)).collect();
    assert_eq!(got, vec![h]);
    assert_eq!(d.payload(), b"after the noise");
}

#[test]
fn clean_line_hello_put_get_list() {
    let (mut c, link) = pair(Faults::default(), 1, 0);
    c.master.line.service.set_name("disk");
    let mut theirs = [0u8; 64];
    let (hello, n) = c.hello("test", &mut theirs).unwrap();
    assert_eq!(&theirs[..n], b"disk");
    assert_eq!((hello.version, hello.role, hello.max_payload as usize), (frame::VERSION, linkproto::app::ROLE_SLAVE, MAX_PAYLOAD));
    for (name, len) in [("empty", 0), ("one", 1), ("chunk", c.chunk), ("odd", 3 * c.chunk + 17)] {
        let bytes = data(len, len as u64);
        assert_eq!(put(&mut c, name, &bytes).unwrap(), sha(&bytes));
        assert_eq!(get(&mut c, name).unwrap(), bytes);
    }
    let mut out = [0u8; MAX_PAYLOAD];
    let (n, next) = c.list(0, &mut out).unwrap();
    assert_eq!(next, None);
    assert_eq!(std::str::from_utf8(&out[..n]).unwrap(), format!("chunk\t{}\nempty\t0\nodd\t{}\none\t1\n", c.chunk, 3 * c.chunk + 17));
    assert_eq!(c.master.stats.retries, 0);
    assert!(link.borrow().turnarounds > 0);
}

#[test]
fn a_slave_slow_to_close_a_put() {
    // The DEXP's stick took seconds to flush a file at PUT_CLOSE (800-LNK-0033): 8 tries of the rate's 300 ms gave up
    // after 2.4 s; each try now waits twice the one before, so 6 s of silence ends with the put kept.
    let (mut c, link) = pair(Faults::default(), 31, 0);
    c.master.timeout_ms = 300;
    c.master.line.service.files.commit_stall = 6_000;
    let bytes = data(3 * c.chunk + 5, 31);
    let start = link.borrow().clock;
    assert_eq!(put(&mut c, "slow", &bytes).unwrap(), sha(&bytes));
    assert!(c.master.stats.retries > 0);
    assert!(link.borrow().clock - start >= 6_000);
    assert_eq!(get(&mut c, "slow").unwrap(), bytes);
    // A slave gone for good is still given up on: here after the 8 tries' 20.5 s (300 ms doubling to 4 s).
    c.master.line.service.files.commit_stall = 60_000;
    assert_eq!(put(&mut c, "gone", &bytes), Err(Error::Timeout));
}

#[test]
fn refusals() {
    let (mut c, _) = pair(Faults::default(), 2, 0);
    assert_eq!(get(&mut c, "missing"), Err(Error::Refused(code::NOT_FOUND)));
    for bad in ["..", ".", "..\\x", "c:x", "tab\there", "../x", "a/../../x", "/abs", "a//b", "a/./b", "a/", "x/.."] {
        assert_eq!(put(&mut c, bad, b"x"), Err(Error::Refused(code::DENIED)), "{bad}");
    }
    assert_eq!(put(&mut c, &"n".repeat(256), b"x"), Err(Error::TooBig));
    // A digest that does not match: refused and the file not kept.
    let mut r = [0u8; 64];
    let mut open = 3u64.to_le_bytes().to_vec();
    open.extend_from_slice(b"bad");
    c.master.call(channel::FILES, kind::PUT_OPEN, &open, &mut r).unwrap();
    let mut chunk = 0u64.to_le_bytes().to_vec();
    chunk.extend_from_slice(b"abc");
    c.master.call(channel::FILES, kind::PUT_DATA, &chunk, &mut r).unwrap();
    assert_eq!(c.master.call(channel::FILES, kind::PUT_CLOSE, &[0; 32], &mut r), Err(Error::Refused(code::MISMATCH)));
    assert_eq!(get(&mut c, "bad"), Err(Error::Refused(code::NOT_FOUND)));
    // Data out of order and an unknown request.
    c.master.call(channel::FILES, kind::PUT_OPEN, &open, &mut r).unwrap();
    let mut late = 1u64.to_le_bytes().to_vec();
    late.push(b'b');
    assert_eq!(c.master.call(channel::FILES, kind::PUT_DATA, &late, &mut r), Err(Error::Refused(code::BAD_REQUEST)));
    assert_eq!(c.master.call(9, 99, &[], &mut r), Err(Error::Refused(code::UNSUPPORTED)));
    assert_eq!(plain_name(b"report.txt"), Some("report.txt"));
    assert_eq!(plain_name(b"a/b"), None);
    assert_eq!(path_name(b"apps/test/x.elf"), Some("apps/test/x.elf"));
    assert_eq!(path_name(&[b'a'; 256]), None);
}

/// 800-LNK-0010's acceptance in the protocol: make a/b/, put a file there, move it to c/, delete a/; ls shows each step.
#[test]
fn directories_over_a_lossy_line() {
    let faults = Faults { lose_write: 20_000, lose_byte: 20, flip_byte: 20, repeat_write: 20_000, echo: true };
    let (mut c, _) = pair(faults, 21, 0);
    c.master.tries = 12;
    let ls = |c: &mut Client<MasterEnd>, dir: &str| -> String {
        let (mut text, mut cursor, mut out) = (String::new(), 0, [0u8; MAX_PAYLOAD]);
        loop {
            let (n, next) = c.list_dir(dir, cursor, &mut out).unwrap();
            text.push_str(std::str::from_utf8(&out[..n]).unwrap());
            match next { Some(n) => cursor = n, None => return text }
        }
    };
    let bytes = data(3000, 22);
    assert_eq!(put(&mut c, "a/b/x.bin", &bytes), Err(Error::Refused(code::NOT_FOUND)));
    c.mkdir("a/b").unwrap();
    c.mkdir("a/b").unwrap();
    assert_eq!(ls(&mut c, ""), "a/\t0\n");
    put(&mut c, "a/b/x.bin", &bytes).unwrap();
    assert_eq!(ls(&mut c, "a/b"), "x.bin\t3000\n");
    c.mkdir("c").unwrap();
    c.rename("a/b/x.bin", "c/x.bin", false).unwrap();
    assert_eq!(ls(&mut c, "a/b"), "");
    assert_eq!(ls(&mut c, "c"), "x.bin\t3000\n");
    assert_eq!(get(&mut c, "c/x.bin").unwrap(), bytes);
    assert_eq!(c.rmdir("c", false), Err(Error::Refused(code::NOT_EMPTY)));
    c.rmdir("a", true).unwrap();
    assert_eq!(ls(&mut c, ""), "c/\t0\n");
    // A file in the way of a move is replaced only when asked; a directory moves whole.
    put(&mut c, "y", b"y").unwrap();
    assert_eq!(c.rename("y", "c/x.bin", false), Err(Error::Refused(code::EXISTS)));
    c.rename("y", "c/x.bin", true).unwrap();
    assert_eq!(get(&mut c, "c/x.bin").unwrap(), b"y");
    c.rename("c", "d", false).unwrap();
    assert_eq!(ls(&mut c, "d"), "x.bin\t1\n");
    for bad in ["../x", "a/../../x", "/x", ""] {
        assert_eq!(c.mkdir(bad), Err(Error::Refused(code::DENIED)), "{bad}");
        assert_eq!(c.rename("d", bad, false), Err(Error::Refused(code::DENIED)), "{bad}");
    }
    assert_eq!(c.rmdir("../d", true), Err(Error::Refused(code::DENIED)));
    assert_eq!(c.list_dir("..", 0, &mut [0; MAX_PAYLOAD]), Err(Error::Refused(code::DENIED)));
    assert_eq!(c.master.line.service.files.dirs.len(), 1);
}

/// 800-LNK-0009 in the protocol: a run started once although its request is repeated, its output read from a cursor
/// past what the slave dropped, its end, a stop.
#[test]
fn programs_over_a_lossy_line() {
    let faults = Faults { lose_write: 30_000, repeat_write: 30_000, echo: true, ..Faults::default() };
    let (mut c, _) = pair(faults, 23, 0);
    c.master.tries = 12;
    put(&mut c, "apps/echo.elf", b"").unwrap_err();
    c.mkdir("apps").unwrap();
    put(&mut c, "apps/echo.elf", b"\x7fELF").unwrap();
    put(&mut c, "loop.elf", b"\x7fELF").unwrap();
    assert_eq!(c.run("missing.elf", ""), Err(Error::Refused(code::NOT_FOUND)));
    assert_eq!(c.run("../echo.elf", ""), Err(Error::Refused(code::DENIED)));
    let mut out = [0u8; MAX_PAYLOAD];
    for i in 0..20 {
        let (run, pid) = c.run("apps/echo.elf", &format!("hello {i}")).unwrap();
        let o = c.output(run, 0, &mut out).unwrap();
        assert_eq!((o.state, o.exit, o.from, &out[..o.len]), (RUN_ENDED, 0, 0, format!("hello {i}\n").as_bytes()), "pid {pid}");
    }
    assert_eq!(c.master.line.service.files.starts, 20, "a repeated RUN started a program twice");
    let (run, _) = c.run("loop.elf", "").unwrap();
    let (mut cursor, mut text) = (0u64, String::new());
    for _ in 0..5 {
        let o = c.output(run, cursor, &mut out).unwrap();
        assert_eq!((o.state, o.exit, o.from), (RUN_RUNNING, EXIT_UNKNOWN, cursor));
        text.push_str(std::str::from_utf8(&out[..o.len]).unwrap());
        cursor = o.from + o.len as u64;
    }
    assert!(text.starts_with("tick 0\ntick 7\n"), "{text}");
    // Output the slave no longer holds is skipped: the answer starts where its bytes do.
    for _ in 0..40 { c.output(run, cursor, &mut out).unwrap(); }
    let o = c.output(run, cursor, &mut out).unwrap();
    assert!(o.from > cursor);
    c.stop(run).unwrap();
    assert_eq!(c.output(run, 0, &mut out).unwrap().state, RUN_STOPPED);
    assert_eq!(c.stop(99), Err(Error::Refused(code::NOT_FOUND)));
    // A restart asked once, though its request is repeated by the line.
    c.reboot().unwrap();
    assert_eq!(c.master.line.service.files.reboots, 1);
    let (n, next) = c.runs(19, &mut out).unwrap();
    assert_eq!((std::str::from_utf8(&out[..n]).unwrap(), next), ("20\t120\tended\tapps/echo.elf\n21\t121\tstopped\tloop.elf\n", None));
}

#[test]
fn replace_over_a_lossy_line() {
    let faults = Faults { lose_write: 30_000, repeat_write: 30_000, echo: true, ..Faults::default() };
    let (mut c, _) = pair(faults, 31, 0);
    c.master.tries = 12;
    c.mkdir("replace").unwrap();
    put(&mut c, "replace/rtc.elf", b"\x7fELF").unwrap();
    for _ in 0..10 {
        c.replace("rtc", "replace/rtc.elf").unwrap();
        c.replace("rtc", "").unwrap();
    }
    let replaced = &c.master.line.service.files.replaced;
    assert!(replaced.len() >= 20 && replaced.iter().all(|(u, _)| u == "rtc"), "{replaced:?}");
    assert_eq!(c.replace("logd", "replace/rtc.elf"), Err(Error::Refused(code::DENIED)));
    assert_eq!(c.replace("rtc", "replace/missing.elf"), Err(Error::Refused(code::NOT_FOUND)));
    assert_eq!(c.replace("rtc", "../rtc.elf"), Err(Error::Refused(code::DENIED)));
    assert_eq!(c.replace("rtc d", ""), Err(Error::Refused(code::BAD_REQUEST)));
    assert_eq!(c.replace("", ""), Err(Error::TooBig));
}

#[test]
fn shell_commands_over_a_lossy_line() {
    let faults = Faults { lose_write: 30_000, repeat_write: 30_000, echo: true, ..Faults::default() };
    let (mut c, _) = pair(faults, 29, 0);
    c.master.tries = 12;
    let mut out = [0u8; MAX_PAYLOAD];
    for i in 0..20 {
        let run = c.command(&format!("ip card {i}")).unwrap();
        let o = c.output(run, 0, &mut out).unwrap();
        assert_eq!((o.state, o.exit, &out[..o.len]), (RUN_ENDED, 0, format!("$ ip card {i}\n").as_bytes()));
    }
    assert_eq!(c.master.line.service.files.commands, 20, "a repeated COMMAND ran a line twice");
    // The shell's refusal is the run's exit and output, not an error of the link.
    let run = c.command("netpolicy add x 1.2.3.4 icmp").unwrap();
    let o = c.output(run, 0, &mut out).unwrap();
    assert_eq!((o.state, o.exit, &out[..o.len]), (RUN_ENDED, 1, &b"refused\n"[..]));
    // Lines the protocol does not carry are refused on either end.
    assert_eq!(c.command("   "), Err(Error::TooBig));
    assert_eq!(c.command(&"x".repeat(241)), Err(Error::TooBig));
    assert_eq!(c.master.call(channel::PROGRAMS, kind::COMMAND, b"ps\nkill 1", &mut out), Err(Error::Refused(code::BAD_REQUEST)));
}

/// INPUT (800-LNK-0032): each frame's words posted once and in order over a line that loses and repeats frames.
#[test]
fn input_over_a_lossy_line() {
    let faults = Faults { lose_write: 30_000, repeat_write: 30_000, echo: true, ..Faults::default() };
    let (mut c, _) = pair(faults, 37, 0);
    c.master.tries = 12;
    let mut sent = Vec::new();
    for i in 0..40u64 {
        let words: Vec<u64> = (0..1 + i % INPUT_MAX as u64).map(|k| (i << 32) | k).collect();
        assert_eq!(c.input(&words).unwrap(), words.len() as u32);
        sent.extend_from_slice(&words);
    }
    assert_eq!(c.master.line.service.files.inputs, sent, "a repeated INPUT posted its events twice, or out of order");
}

/// INPUT's bounds: 1 to INPUT_MAX words, whole words, within the payload agreed; a slave without input refuses it.
#[test]
fn input_bounds_and_an_end_without_input() {
    let (mut c, _) = pair(Faults::default(), 41, 0);
    assert_eq!(c.input(&[]), Err(Error::TooBig));
    assert_eq!(c.input(&[7; INPUT_MAX + 1]), Err(Error::TooBig));
    assert_eq!(c.input(&[0, u64::MAX]), Ok(2));
    let mut r = [0u8; MAX_PAYLOAD];
    for bad in [&[][..], &[1u8; 12][..], &[1u8; 8 * (INPUT_MAX + 1)][..]] {
        assert_eq!(c.master.call(channel::INPUT, kind::INPUT, bad, &mut r), Err(Error::Refused(code::BAD_REQUEST)), "{} bytes", bad.len());
    }
    assert_eq!(c.master.line.service.files.inputs, [0, u64::MAX]);
    // At the smallest payload a frame holds 8 events.
    c.max_payload = MIN_PAYLOAD;
    c.hello("small", &mut [0; 64]).unwrap();
    assert_eq!(c.input_max(), 8);
    assert_eq!(c.input(&[1; 9]), Err(Error::TooBig));
    assert_eq!(c.input(&[1; 8]), Ok(8));
    // An end whose `Files` posts no input answers UNSUPPORTED, as a slave from before INPUT's payload did.
    struct NoInput;
    impl Files for NoInput {
        fn create(&mut self, _: &str, _: u64) -> Result<(), u8> { Err(code::IO) }
        fn write_at(&mut self, _: u64, _: &[u8]) -> Result<(), u8> { Err(code::IO) }
        fn commit(&mut self, _: &[u8; 32], _: bool) -> Result<(), u8> { Err(code::IO) }
        fn open(&mut self, _: &str) -> Result<u64, u8> { Err(code::NOT_FOUND) }
        fn read_at(&mut self, _: u64, _: &mut [u8]) -> Result<usize, u8> { Err(code::IO) }
        fn list(&mut self, _: u32, _: &mut [u8]) -> Result<(usize, Option<u32>), u8> { Ok((0, None)) }
    }
    let mut service = FileService::new(NoInput);
    let mut reply = [0u8; MAX_PAYLOAD];
    assert_eq!(service.handle(channel::INPUT, kind::INPUT, &[0; 8], &mut reply), Err(code::UNSUPPORTED));
}

#[test]
fn log_and_console() {
    let (mut c, _) = pair(Faults::default(), 3, 0);
    c.master.line.service.files.log = b"boot\nready\n".to_vec();
    let mut out = [0u8; MAX_PAYLOAD];
    let (n, next) = c.log(0, &mut out).unwrap();
    assert_eq!((&out[..n], next), (&b"boot\nready\n"[..], 11));
    assert_eq!(c.log(next, &mut out).unwrap(), (0, 11));
    let n = c.console(b"ls\n", &mut out).unwrap();
    assert_eq!(&out[..n], b"LS\n");
    assert_eq!(c.master.line.service.files.console, b"ls\n");
}

/// A master set to small frames sends and gets no larger ones.
#[test]
fn small_frames_set_by_the_master() {
    let (mut c, _) = pair(Faults::default(), 11, 0);
    c.max_payload = 256;
    c.hello("small", &mut [0; 64]).unwrap();
    assert_eq!(c.chunk, 256 - 8);
    let bytes = data(5000, 14);
    put(&mut c, "s", &bytes).unwrap();
    assert_eq!(get(&mut c, "s").unwrap(), bytes);
}

/// A master that takes small payloads gets no larger answer.
#[test]
fn small_master_payload() {
    let (mut c, _) = pair(Faults::default(), 10, 0);
    c.master.line.service.files.log = data(3000, 13);
    let mut r = [0u8; MAX_PAYLOAD];
    c.master.call(channel::CONTROL, kind::HELLO, &[1, 0, 100, 0], &mut r).unwrap();
    let mut out = [0u8; MAX_PAYLOAD];
    assert_eq!(c.log(0, &mut out).unwrap(), (92, 92));
    c.master.call(channel::CONTROL, kind::HELLO, &[1, 0, 1, 0], &mut r).unwrap();
    assert_eq!(c.log(0, &mut out).unwrap().0, linkproto::app::MIN_PAYLOAD - 8);
}

/// A mebibyte each way over a line that loses and damages frames, starting near the sequence numbers' wrap: whole and
/// in order, each chunk written once.
#[test]
fn lossy_line_megabyte() {
    let faults = Faults { lose_write: 30_000, lose_byte: 20, flip_byte: 20, repeat_write: 20_000, echo: false };
    let (mut c, _) = pair(faults, 0xC0FFEE, 65_000);
    c.master.timeout_ms = 50;
    c.master.tries = 12;
    c.hello("lossy", &mut [0; 64]).unwrap();
    let bytes = data(1 << 20, 99);
    assert_eq!(put(&mut c, "big.bin", &bytes).unwrap(), sha(&bytes));
    let chunks = (bytes.len() + c.chunk - 1) / c.chunk;
    assert_eq!(c.master.line.service.files.writes as usize, chunks, "a repeated request was carried out twice");
    assert_eq!(c.master.line.service.files.files["big.bin"], bytes);
    assert_eq!(get(&mut c, "big.bin").unwrap(), bytes);
    let stats = c.master.stats;
    assert!(stats.retries > 0 && c.master.line.slave.stats.retries > 0, "the faults never struck: {stats:?}");
}

/// An RS-485 converter that hears what it sends: each end ignores its own frames.
#[test]
fn echoing_line() {
    let faults = Faults { echo: true, ..Faults::default() };
    let (mut c, _) = pair(faults, 4, 0);
    c.hello("echo", &mut [0; 64]).unwrap();
    let bytes = data(10_000, 5);
    put(&mut c, "e", &bytes).unwrap();
    assert_eq!(get(&mut c, "e").unwrap(), bytes);
    assert_eq!(c.master.stats.retries, 0);
}

/// A line that carries nothing: the master gives up after its tries.
#[test]
fn dead_line_times_out() {
    let faults = Faults { lose_write: 1_000_000, ..Faults::default() };
    let (mut c, link) = pair(faults, 6, 0);
    c.master.timeout_ms = 100;
    c.master.tries = 3;
    assert_eq!(c.hello("dead", &mut [0; 64]).map(|_| ()), Err(Error::Timeout));
    assert_eq!(c.master.stats.retries, 2);
    // The tries wait 100, 200 and 400 ms, each twice the one before (800-LNK-0033).
    assert_eq!(link.borrow().clock, 700);
}

/// The line dies in the middle of a put; the next put of the same file goes on from where the slave stopped.
#[test]
fn interrupted_put_resumes() {
    let (mut c, link) = pair(Faults::default(), 8, 0);
    c.master.timeout_ms = 20;
    c.master.tries = 2;
    let bytes = data(50 * 1000, 11);
    let cut = link.clone();
    let failed = c.put("r.bin", bytes.len() as u64, |off, out| {
        if off > 20_000 { cut.borrow_mut().wire.faults.lose_write = 1_000_000; }
        let rest = &bytes[off as usize..]; let n = rest.len().min(out.len()); out[..n].copy_from_slice(&rest[..n]); n
    });
    assert_eq!(failed, Err(Error::Timeout));
    link.borrow_mut().wire.faults.lose_write = 0;
    let read = |off: u64, out: &mut [u8]| { let rest = &bytes[off as usize..]; let n = rest.len().min(out.len()); out[..n].copy_from_slice(&rest[..n]); n };
    let (digest, resumed) = c.put("r.bin", bytes.len() as u64, read).unwrap();
    assert!(resumed > 20_000 && resumed < bytes.len() as u64, "resumed from {resumed}");
    assert_eq!(digest, sha(&bytes));
    let chunks = (bytes.len() + c.chunk - 1) / c.chunk;
    assert_eq!(c.master.line.service.files.writes as usize, chunks);
    assert_eq!(get(&mut c, "r.bin").unwrap(), bytes);
    // Another size or name starts afresh.
    let (_, resumed) = c.put("r.bin", 3, |_, out| { out[..3].copy_from_slice(b"new"); 3 }).unwrap();
    assert_eq!(resumed, 0);
    assert_eq!(get(&mut c, "r.bin").unwrap(), b"new");
}

#[test]
fn stat_and_delete() {
    let (mut c, _) = pair(Faults::default(), 9, 0);
    let bytes = data(5000, 12);
    put(&mut c, "s", &bytes).unwrap();
    assert_eq!(c.stat("s").unwrap(), (5000, sha(&bytes)));
    c.delete("s").unwrap();
    assert_eq!(c.stat("s"), Err(Error::Refused(code::NOT_FOUND)));
    assert_eq!(c.delete("s"), Err(Error::Refused(code::NOT_FOUND)));
    assert_eq!(c.delete("../s"), Err(Error::Refused(code::DENIED)));
}

/// 100 000 damaged frames through the decoder and 100 000 random requests to the service: no panic, and nothing damaged
/// is taken for a frame (500-ASR-0001's way).
#[test]
fn fuzz_decoder_and_service() {
    let mut rng = Rng(0xF0F0_1234_5678_9ABC);
    let mut d = Decoder::new();
    let mut wire = [0u8; MAX_WIRE];
    let mut accepted = 0;
    for i in 0..100_000u32 {
        let len = (rng.next() % 300) as usize;
        let payload: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        let h = Header { address: 0, channel: rng.next() as u8, kind: rng.next() as u8, flags: 0, seq: i as u16 };
        let n = frame::encode(&h, &payload, &mut wire).unwrap();
        let mut bytes = wire[..n].to_vec();
        match rng.next() % 4 {
            0 => for _ in 0..1 + rng.next() % 4 { let at = (rng.next() as usize) % bytes.len(); bytes[at] = rng.next() as u8; },
            1 => bytes.truncate((rng.next() as usize) % bytes.len()),
            2 => { let at = (rng.next() as usize) % bytes.len(); bytes.insert(at, rng.next() as u8); }
            _ => {}
        }
        bytes.push(0);
        for &b in &bytes {
            if let Some(got) = d.push(b) {
                assert!(got == h && d.payload() == &payload[..], "a damaged frame taken for a good one at {i}");
                accepted += 1;
            }
        }
    }
    assert!(accepted > 20_000, "only {accepted} frames came through");

    let mut service = FileService::new(Disk::default());
    let kinds = [kind::HELLO, kind::POLL, kind::REBOOT, kind::PUT_OPEN, kind::PUT_DATA, kind::PUT_CLOSE, kind::GET_OPEN, kind::GET_DATA,
                 kind::GET_CLOSE, kind::LIST, kind::STAT, kind::DELETE, kind::CONSOLE, kind::LOG, kind::INPUT, kind::MKDIR, kind::MOVE,
                 kind::RMDIR, kind::LIST_DIR, kind::RUN, kind::OUTPUT, kind::STOP, kind::RUNS, 0x55];
    let mut reply = [0u8; MAX_PAYLOAD];
    for _ in 0..100_000 {
        let k = kinds[(rng.next() as usize) % kinds.len()];
        let ch = if rng.chance(900_000) { [channel::CONTROL, channel::FILES, channel::FILES, channel::LOG, channel::CONSOLE, channel::PROGRAMS][(rng.next() % 6) as usize] } else { rng.next() as u8 };
        let len = (rng.next() % 64) as usize;
        let mut p: Vec<u8> = (0..len).map(|_| if rng.chance(500_000) { b'a' + (rng.next() % 3) as u8 } else { rng.next() as u8 }).collect();
        // Small offsets and sizes, so data requests sometimes fit.
        if p.len() >= 8 && rng.chance(700_000) { p[..8].copy_from_slice(&(rng.next() % 64).to_le_bytes()); }
        let _ = service.handle(ch, k, &p, &mut reply);
    }
}

// 800-LNK-0026: several slaves on one line. Every byte one end sends reaches every other end, as on an RS-485 pair.
struct Bus { queues: Vec<VecDeque<u8>>, clock: u64 }
type SharedBus = Rc<RefCell<Bus>>;
fn hear_all(bus: &SharedBus, from: usize, bytes: &[u8]) {
    let b = &mut *bus.borrow_mut();
    for (i, q) in b.queues.iter_mut().enumerate() { if i != from { q.extend(bytes); } }
}
struct BusSlaveEnd(SharedBus, usize);
impl Line for BusSlaveEnd {
    fn write(&mut self, bytes: &[u8]) -> bool { hear_all(&self.0, self.1, bytes); true }
    fn read(&mut self, out: &mut [u8], _timeout_ms: u32) -> usize { take(&mut self.0.borrow_mut().queues[self.1], out) }
    fn now_ms(&mut self) -> u64 { self.0.borrow().clock }
}
/// The master is end 0; the slaves are served whenever it waits, until none has bytes left to read.
struct BusMasterEnd { bus: SharedBus, slaves: Vec<(Slave<BusSlaveEnd>, FileService<Disk>)> }
impl Line for BusMasterEnd {
    fn write(&mut self, bytes: &[u8]) -> bool { hear_all(&self.bus, 0, bytes); true }
    fn read(&mut self, out: &mut [u8], timeout_ms: u32) -> usize {
        while self.slaves.iter().any(|(s, _)| !self.bus.borrow().queues[s.line.1].is_empty()) {
            for (slave, service) in self.slaves.iter_mut() { slave.serve(service, 0).unwrap(); }
        }
        let n = take(&mut self.bus.borrow_mut().queues[0], out);
        self.bus.borrow_mut().clock += if n == 0 { timeout_ms as u64 } else { 1 };
        n
    }
    fn now_ms(&mut self) -> u64 { self.bus.borrow().clock }
}

/// A master on a line with slaves of these addresses (0 is a slave from before addresses), each named `s<address>`.
fn bus(addresses: &[u8]) -> Client<BusMasterEnd> {
    let bus = Rc::new(RefCell::new(Bus { queues: vec![VecDeque::new(); addresses.len() + 1], clock: 0 }));
    let slaves = addresses.iter().enumerate().map(|(i, &a)| {
        let mut slave = Slave::new(BusSlaveEnd(bus.clone(), i + 1));
        slave.address = a;
        let mut service = FileService::new(Disk::default());
        service.set_name(&format!("s{a}"));
        (slave, service)
    }).collect();
    Client::new(BusMasterEnd { bus, slaves })
}

#[test]
fn a_shared_line_with_three_addressed_slaves_and_an_old_one() {
    let mut c = bus(&[1, 2, 3, 0]);
    c.master.timeout_ms = 50;
    c.master.tries = 2;
    let mut theirs = [0u8; 64];
    // Each address is answered by its slave alone; files go to it and nowhere else.
    for a in [1u8, 2, 3] {
        c.master.address = a;
        let (_, n) = c.hello("master", &mut theirs).unwrap();
        assert_eq!(&theirs[..n], format!("s{a}").as_bytes());
        let bytes = data(3 * c.chunk + a as usize, a as u64);
        assert_eq!(put(&mut c, &format!("for{a}"), &bytes).unwrap(), sha(&bytes));
        assert_eq!(get(&mut c, &format!("for{a}")).unwrap(), bytes);
    }
    for (i, a) in [1u8, 2, 3].iter().enumerate() {
        let files = &c.master.line.slaves[i].1.files.files;
        assert_eq!(files.keys().cloned().collect::<Vec<_>>(), vec![format!("for{a}")]);
    }
    // The old slave heard every version 2 frame and dropped it: it answered nothing, and holds nothing.
    let old = &c.master.line.slaves[3];
    assert_eq!(old.0.stats.requests, 0);
    assert!(old.1.files.files.is_empty());
    // Without an address the master speaks version 1, and only the old slave answers it.
    c.master.address = 0;
    let (_, n) = c.hello("master", &mut theirs).unwrap();
    assert_eq!(&theirs[..n], b"s0");
    assert_eq!(c.master.line.slaves.iter().map(|(s, _)| s.stats.requests).collect::<Vec<_>>(), vec![
        c.master.line.slaves[0].0.stats.requests, c.master.line.slaves[1].0.stats.requests, c.master.line.slaves[2].0.stats.requests, 1]);
    // An address nobody has: no answer, and no slave answered in its place.
    c.master.address = 9;
    assert_eq!(c.hello("master", &mut theirs).err(), Some(Error::Timeout));
    assert_eq!(c.master.stats.retries, 1);
}

#[test]
fn an_addressed_frame_round_trip_and_its_limits() {
    let h = Header { address: 5, channel: channel::FILES, kind: kind::PUT_DATA, flags: 0, seq: 0x1234 };
    let mut wire = [0u8; MAX_WIRE];
    let n = frame::encode(&h, b"to five", &mut wire).unwrap();
    let mut d = Decoder::new();
    assert_eq!(wire[..n].iter().filter_map(|&b| d.push(b)).collect::<Vec<_>>(), vec![h]);
    assert_eq!(d.payload(), b"to five");
    // Beyond the last address, nothing is framed.
    assert_eq!(frame::encode(&Header { address: frame::ADDRESS_MAX + 1, ..h }, b"", &mut wire), None);
    // A version 2 frame whose address byte is 0 is dropped, even with a good CRC.
    let mut raw = vec![frame::VERSION_ADDRESSED, 0, h.channel, h.kind, 0, 0x34, 0x12, 0, 0];
    raw.extend_from_slice(&crc::crc32c(&raw).to_le_bytes());
    let mut coded = vec![0u8; MAX_WIRE];
    let m = cobs::encode(&raw, &mut coded).unwrap();
    coded[m] = 0;
    let before = d.dropped;
    assert!(coded[..=m].iter().filter_map(|&b| d.push(b)).next().is_none());
    assert_eq!(d.dropped, before + 1);
}
