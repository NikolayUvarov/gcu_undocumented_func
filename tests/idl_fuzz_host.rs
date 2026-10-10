//! 500-ASR-0001: seeded fuzzing of every generated MIND IDL decoder on the host. Each interface's receiver `decode` and
//! each generated type's `Wire::decode` run on random and mutated input, and must never panic, must release a received
//! capability (MC-2.12), must decode a buffer call from its private copy (MC-2.11), must refuse a wrong major version,
//! trailing or missing bytes, and must accept only canonical encodings (MC-2.4). A run is evidence of the executions it
//! made, not a proof (MC-12.2). MIND_FUZZ_SEED and MIND_FUZZ_ITERATIONS override the fixed seed and count.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/arch/mod.rs"]
mod arch;
#[path = "../libmind/src/sys.rs"]
mod sys;

mod dev {
    pub fn cap_info(slot: usize) -> (usize, usize, usize) { (crate::ipc::kind(slot), 0, 0) }
}

mod mem {
    /// Client buffers: never shared here, since no client call runs.
    pub struct Pages(Vec<u8>);
    impl Pages {
        pub fn new(bytes: usize) -> Option<Self> { Some(Self(vec![0; bytes])) }
        pub fn len(&self) -> usize { self.0.len() }
        pub fn as_slice(&self) -> &[u8] { &self.0 }
        pub fn as_mut_slice(&mut self) -> &mut [u8] { &mut self.0 }
        pub fn share(&self) -> crate::sys::Result<usize> { Err(crate::sys::Error::Rights) }
    }
    /// The server's view of the memory capability in a slot: the fuzzer's client buffer.
    pub struct Mapping { address: usize, length: usize }
    impl Mapping {
        pub fn new(slot: usize) -> crate::sys::Result<Self> {
            let (address, length) = crate::ipc::memory(slot).ok_or(crate::sys::Error::Rights)?;
            Ok(Self { address, length })
        }
        pub fn len(&self) -> usize { self.length }
        pub fn as_slice(&self) -> &[u8] { unsafe { std::slice::from_raw_parts(self.address as *const u8, self.length) } }
        pub fn as_mut_slice(&mut self) -> &mut [u8] { unsafe { std::slice::from_raw_parts_mut(self.address as *mut u8, self.length) } }
    }
}

mod ipc {
    use crate::abi::*;
    use std::cell::{Cell, RefCell};
    #[derive(Clone, Copy, Debug, Default)]
    pub struct Message { pub data: [usize; 2], pub cap: usize, pub moved: bool }
    impl Message {
        pub fn new(a: usize, b: usize) -> Self { Self { data: [a, b], cap: 0, moved: false } }
        pub fn with_cap(mut self, cap: usize, _rights: u8) -> Self { self.cap = cap; self }
        pub fn with_cap_moved(mut self, cap: usize, _rights: u8) -> Self { self.cap = cap; self.moved = true; self }
    }
    #[derive(Clone, Copy, Debug)]
    pub struct Received { pub data: [usize; 2], pub sender: u64, pub badge: u16, pub cap_received: bool, pub is_call: bool, pub irq: Option<usize> }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Endpoint(pub usize);
    impl Endpoint {
        /// No server answers: only receivers are fuzzed here.
        pub fn call(&self, _message: &Message, _receive: usize) -> crate::sys::Result<Received> { Err(crate::sys::Error::Peer) }
        /// `wire::call` waits with a timeout when a client sets one (`wire::with_timeout`, 211-APP-0044).
        pub fn call_timeout(&self, message: &Message, receive: usize, _ms: u32) -> crate::sys::Result<Received> { self.call(message, receive) }
    }
    pub const SERVER_SLOT: usize = 9;
    thread_local! {
        // What the server's receive slot holds: kind and, for memory, (address, length).
        pub static SLOT: Cell<(usize, Option<(usize, usize)>)> = Cell::new((CAP_KIND_NONE, None));
        pub static DROPPED: RefCell<Vec<usize>> = RefCell::new(Vec::new());
    }
    pub fn kind(slot: usize) -> usize { if slot == SERVER_SLOT { SLOT.with(|s| s.get().0) } else { CAP_KIND_NONE } }
    pub fn memory(slot: usize) -> Option<(usize, usize)> { if slot == SERVER_SLOT { SLOT.with(|s| s.get().1) } else { None } }
    pub fn reply(_message: &Message) -> crate::sys::Result<()> { Ok(()) }
    pub fn save_reply() -> crate::sys::Result<usize> { Ok(30) }
    pub fn reply_saved(_slot: usize, _message: &Message) -> crate::sys::Result<()> { Ok(()) }
    pub fn drop_cap(slot: usize) -> crate::sys::Result<()> { DROPPED.with(|d| d.borrow_mut().push(slot)); Ok(()) }
    pub fn revoke(_slot: usize) -> crate::sys::Result<usize> { Ok(1) }
}

mod idl {
    #[path = "../../libmind/src/idl/codec.rs"] pub mod codec;
    #[path = "../../libmind/src/idl/wire.rs"] pub mod wire;
    #[path = "../../libmind/src/idl/audio.rs"] pub mod audio;
    #[path = "../../libmind/src/idl/block.rs"] pub mod block;
    #[path = "../../libmind/src/idl/blockstore.rs"] pub mod blockstore;
    #[path = "../../libmind/src/idl/display.rs"] pub mod display;
    #[path = "../../libmind/src/idl/gpio.rs"] pub mod gpio;
    #[path = "../../libmind/src/idl/init.rs"] pub mod init;
    #[path = "../../libmind/src/idl/keyboard.rs"] pub mod keyboard;
    #[path = "../../libmind/src/idl/keystore.rs"] pub mod keystore;
    #[path = "../../libmind/src/idl/loader.rs"] pub mod loader;
    #[path = "../../libmind/src/idl/log.rs"] pub mod log;
    #[path = "../../libmind/src/idl/net.rs"] pub mod net;
    #[path = "../../libmind/src/idl/netpolicy.rs"] pub mod netpolicy;
    #[path = "../../libmind/src/idl/parse.rs"] pub mod parse;
    #[path = "../../libmind/src/idl/rtc.rs"] pub mod rtc;
    #[path = "../../libmind/src/idl/shell.rs"] pub mod shell;
    #[path = "../../libmind/src/idl/socket.rs"] pub mod socket;
    #[path = "../../libmind/src/idl/sysinfo.rs"] pub mod sysinfo;
    #[path = "../../libmind/src/idl/tls.rs"] pub mod tls;
    #[path = "../../libmind/src/idl/tpm.rs"] pub mod tpm;
    #[path = "../../libmind/src/idl/tts.rs"] pub mod tts;
    #[path = "../../libmind/src/idl/usb.rs"] pub mod usb;
    #[path = "../../libmind/src/idl/vfs.rs"] pub mod vfs;
    #[path = "../../libmind/src/idl/video.rs"] pub mod video;
    #[path = "../../libmind/src/idl/voice.rs"] pub mod voice;
    #[path = "../../libmind/src/idl/window.rs"] pub mod window;
}

use abi::*;
use idl::codec::{Reader, Wire, Writer};
use idl::wire::Reject;
use std::panic::{self, AssertUnwindSafe};

const SEED: u64 = 0x5000_A5A0_0001;
const ITERATIONS: usize = 50_000;

fn seed() -> u64 { std::env::var("MIND_FUZZ_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(SEED) }
fn iterations() -> usize { std::env::var("MIND_FUZZ_ITERATIONS").ok().and_then(|s| s.parse().ok()).unwrap_or(ITERATIONS) }

/// xorshift64*: the same seed gives the same inputs on every host.
struct Rng(u64);
impl Rng {
    fn new(seed: u64, stream: &str) -> Self { Self(stream.bytes().fold(seed ^ 0x9E37_79B9_7F4A_7C15, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01B3)) | 1) }
    fn next(&mut self) -> u64 { self.0 ^= self.0 >> 12; self.0 ^= self.0 << 25; self.0 ^= self.0 >> 27; self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) }
    fn below(&mut self, n: usize) -> usize { if n == 0 { 0 } else { (self.next() % n as u64) as usize } }
    fn chance(&mut self, percent: usize) -> bool { self.below(100) < percent }
    fn byte(&mut self) -> u8 { const SPECIAL: [u8; 8] = [0, 1, 2, 0x7F, 0x80, 0xFE, 0xFF, b'a']; if self.chance(40) { SPECIAL[self.below(8)] } else { self.next() as u8 } }
}

/// Byte-level mutations of a corpus input; `limit` caps the length.
fn mutate(rng: &mut Rng, input: &[u8], donor: &[u8], limit: usize) -> Vec<u8> {
    let mut out = input.to_vec();
    for _ in 0..1 + rng.below(4) {
        match rng.below(8) {
            0 if !out.is_empty() => { let i = rng.below(out.len()); out[i] ^= 1 << rng.below(8); }
            1 if !out.is_empty() => { let i = rng.below(out.len()); out[i] = rng.byte(); }
            2 if out.len() >= 2 => {
                // A u16 length or count at a random place: around the bounds the schema states.
                const VALUES: [u16; 8] = [0, 1, 2, 3, 127, 128, 0x7FFF, 0xFFFF];
                let i = rng.below(out.len() - 1); let v = if rng.chance(50) { VALUES[rng.below(8)] } else { rng.next() as u16 };
                out[i..i + 2].copy_from_slice(&v.to_le_bytes());
            }
            3 if !out.is_empty() => { let n = rng.below(out.len()) + 1; out.truncate(out.len() - n.min(1 + rng.below(4))); }
            4 => { for _ in 0..1 + rng.below(8) { out.push(rng.byte()); } }
            5 if !out.is_empty() => { let i = rng.below(out.len()); out.insert(i, rng.byte()); }
            6 if !out.is_empty() => { let i = rng.below(out.len()); out.remove(i); }
            7 if !donor.is_empty() => { let at = rng.below(out.len() + 1); let from = rng.below(donor.len()); let n = 1 + rng.below(donor.len() - from); out.splice(at..at, donor[from..from + n].iter().copied()); }
            _ => { out.push(rng.byte()); }
        }
    }
    out.truncate(limit);
    out
}

/// A corpus of inputs a decoder accepted, grown from what mutation finds.
struct Corpus(Vec<Vec<u8>>);
impl Corpus {
    fn add(&mut self, input: &[u8]) { if self.0.len() < 512 && !self.0.iter().any(|e| e == input) { self.0.push(input.to_vec()); } }
    fn pick<'a>(&'a self, rng: &mut Rng) -> &'a [u8] { if self.0.is_empty() { &[] } else { &self.0[rng.below(self.0.len())] } }
}

/// Runs `f`, turning a panic into its message.
fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(|e| e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default())
}

fn hex(bytes: &[u8]) -> String { bytes.iter().take(64).map(|b| format!("{b:02x}")).collect::<String>() + if bytes.len() > 64 { "…" } else { "" } }

/// Findings of one target, reported together with the seed that reproduces them.
#[derive(Default)]
struct Findings(Vec<String>);
impl Findings {
    fn add(&mut self, text: String) { if self.0.len() < 20 { self.0.push(text); } }
}

// --- Receivers: `decode` of every interface ---------------------------------------------------------------------

/// One received message: its words, whether it is a call, an interrupt, and the capability it carried.
#[derive(Clone, Debug)]
struct Message { words: [usize; 2], is_call: bool, irq: bool, cap: Option<usize>, buffer: Vec<u8> }

/// What `decode` gave: the request's Debug text or the rejection, and the slots it dropped.
#[derive(Clone, Debug, PartialEq)]
struct Outcome { decoded: Result<String, Reject>, dropped: Vec<usize>, changed_by_client: bool }

/// Delivers `message` to a receiver and runs `decode` on it. For buffer calls `decode` also writes the client's buffer
/// after decoding, and reports whether that changed the request (it must not: MC-2.11).
fn deliver(message: &Message, decode: &dyn Fn(&ipc::Received) -> (Result<String, Reject>, bool)) -> Outcome {
    let mut buffer = message.buffer.clone();
    let memory = (message.cap == Some(CAP_KIND_MEMORY)).then(|| (buffer.as_mut_ptr() as usize, buffer.len()));
    ipc::SLOT.with(|s| s.set((message.cap.unwrap_or(CAP_KIND_NONE), memory)));
    ipc::DROPPED.with(|d| d.borrow_mut().clear());
    let received = ipc::Received { data: message.words, sender: 2, badge: 0, cap_received: message.cap.is_some(), is_call: message.is_call, irq: message.irq.then_some(1) };
    let (decoded, changed_by_client) = decode(&received);
    drop(buffer);
    Outcome { decoded, dropped: ipc::DROPPED.with(|d| d.borrow().clone()), changed_by_client }
}

/// Checks one message against the receiver's rules, then the variants an accepted message must not pass.
fn check_receiver(name: &str, major: usize, buffer_calls: bool, message: &Message, decode: &dyn Fn(&ipc::Received) -> (Result<String, Reject>, bool), findings: &mut Findings) -> bool {
    let run = |m: &Message| guarded(|| deliver(m, decode));
    let report = |what: &str, m: &Message, findings: &mut Findings| findings.add(format!("{name}: {what}: words {:#x} {:#x}, call {}, irq {}, cap {:?}, buffer {} [{}]", m.words[0], m.words[1], m.is_call, m.irq, m.cap, m.buffer.len(), hex(&m.buffer)));
    let first = match run(message) { Ok(o) => o, Err(p) => { report(&format!("panicked ({p})"), message, findings); return false; } };
    if first.changed_by_client { report("the request changed when the client wrote its buffer after decoding (MC-2.11)", message, findings); }
    if message.cap.is_some() && !first.dropped.contains(&ipc::SERVER_SLOT) { report("the received capability was not released (MC-2.12)", message, findings); }
    match run(message) { Ok(again) if again == first => {} _ => report("decoding the same message twice gave different results", message, findings) }
    let Ok(_) = first.decoded else { return false };
    if !message.is_call || message.irq { report("accepted a message that is not a call", message, findings); }
    // Another major version must be refused as such.
    let mut other = message.clone();
    other.words[0] ^= (1 + (message.words[0] >> 3) % 255) << 8;
    if (other.words[0] >> 8) & 0xFF != major {
        if let Ok(o) = run(&other) { if o.decoded != Err(Reject::Version) { report("a wrong major version was not refused as Version", &other, findings); } }
    }
    if buffer_calls && message.cap == Some(CAP_KIND_MEMORY) {
        let length = message.words[0] >> 16;
        // One byte more than the request is trailing data; one byte less cuts the last field.
        let mut longer = message.clone();
        longer.words[0] += 1 << 16;
        if longer.buffer.len() <= length { longer.buffer.push(0); }
        if let Ok(o) = run(&longer) { if o.decoded.is_ok() { report("accepted a request with a trailing byte", &longer, findings); } }
        if length > 0 {
            let mut shorter = message.clone();
            shorter.words[0] -= 1 << 16;
            if let Ok(o) = run(&shorter) { if o.decoded.is_ok() { report("accepted a request with its last byte cut", &shorter, findings); } }
        }
    }
    true
}

/// The shape of an interface's receiver, read from its generated source: its methods, the largest private copy of a
/// buffer request, and the largest reply room a buffer call asks of the client's buffer.
struct Shape { methods: Vec<usize>, buffer_methods: Vec<usize>, copy_max: usize, reply_max: usize }
impl Shape {
    fn buffer_calls(&self) -> bool { self.copy_max > 0 || self.reply_max > 0 }
    fn room(&self) -> usize { self.copy_max.max(self.reply_max) + 16 }
}
fn shape(module: &str) -> Shape {
    let text = std::fs::read_to_string(idl_dir().join(format!("{module}.rs"))).unwrap();
    let number = |rest: &str| rest.split(|c: char| !c.is_ascii_digit()).next().and_then(|n| n.parse::<usize>().ok());
    let mut shape = Shape { methods: Vec::new(), buffer_methods: Vec::new(), copy_max: 0, reply_max: 0 };
    let decode = &text[text.find("pub fn decode").expect("a decode function")..];
    let decode = &decode[..decode.find("_ => { wire::discard").unwrap_or(decode.len())];
    for line in decode.lines() {
        let t = line.trim_start();
        if line.starts_with("        ") && !line.starts_with("         ") { if let Some(rest) = t.strip_suffix(" => {") { if let Some(n) = number(rest) { shape.methods.push(n); } } }
        if let Some(i) = t.find("take_buffer(request, cap, ") {
            if let Some(n) = number(&t[i + 26..]) { shape.reply_max = shape.reply_max.max(n); }
            if let Some(&method) = shape.methods.last() { if !shape.buffer_methods.contains(&method) { shape.buffer_methods.push(method); } }
        }
        if let Some(rest) = t.strip_prefix("let mut copy = [0u8; ") { if let Some(n) = number(rest) { shape.copy_max = shape.copy_max.max(n); } }
    }
    for line in text.lines() { if let Some(rest) = line.strip_prefix("pub const REQUEST_MAX: usize = ") { shape.copy_max = shape.copy_max.max(number(rest).unwrap()); } }
    shape
}

/// A capability kind for a message: none, an endpoint, memory (with a buffer) or any kind.
fn cap_kind(rng: &mut Rng, buffer_calls: bool) -> Option<usize> {
    match rng.below(10) { 0..=3 => None, 4..=6 if buffer_calls => Some(CAP_KIND_MEMORY), 7 => Some(CAP_KIND_ENDPOINT), _ => Some(rng.below(16)) }
}

/// Fuzzes the receiver of one interface.
fn fuzz_receiver(name: &str, major: usize, shape: &Shape, decode: &dyn Fn(&ipc::Received) -> (Result<String, Reject>, bool)) -> (usize, Findings) {
    let mut rng = Rng::new(seed(), name);
    let mut findings = Findings::default();
    let mut accepted = 0;
    // Accepted word messages and their capability kind; accepted buffer payloads.
    let mut words_corpus: Vec<([usize; 2], Option<usize>)> = Vec::new();
    let mut buffers = Corpus(Vec::new());
    let buffer_calls = shape.buffer_calls();
    let room = shape.room();
    let try_message = |m: &Message, words_corpus: &mut Vec<([usize; 2], Option<usize>)>, buffers: &mut Corpus, findings: &mut Findings| {
        let memory = m.cap == Some(CAP_KIND_MEMORY);
        let buffer_method = memory && shape.buffer_methods.contains(&(m.words[0] & 0xFF));
        if !check_receiver(name, major, buffer_method, m, decode, findings) { return false; }
        if buffer_method { buffers.add(&m.buffer[..(m.words[0] >> 16).min(m.buffer.len())]); }
        if words_corpus.len() < 512 { words_corpus.push((m.words, m.cap)); }
        true
    };
    // Seeds: each method with no fields under every capability kind, and zeroed buffer requests of each length.
    for &method in &shape.methods {
        for cap in [None].into_iter().chain((0..16).map(Some)) {
            let buffer = if cap == Some(CAP_KIND_MEMORY) { vec![0; room] } else { Vec::new() };
            let m = Message { words: [method | major << 8, 0], is_call: true, irq: false, cap, buffer };
            if try_message(&m, &mut words_corpus, &mut buffers, &mut findings) { accepted += 1; }
        }
        if buffer_calls {
            for length in 1..=shape.copy_max.min(512) {
                let m = Message { words: [method | major << 8 | length << 16, 0], is_call: true, irq: false, cap: Some(CAP_KIND_MEMORY), buffer: vec![0; room.max(length)] };
                if try_message(&m, &mut words_corpus, &mut buffers, &mut findings) { accepted += 1; }
            }
        }
    }
    for _ in 0..iterations() {
        let mut m = Message { words: [0; 2], is_call: !rng.chance(3), irq: rng.chance(2), cap: None, buffer: Vec::new() };
        if !words_corpus.is_empty() && rng.chance(50) {
            let (w, cap) = words_corpus[rng.below(words_corpus.len())];
            m.words = w; m.cap = if rng.chance(80) { cap } else { cap_kind(&mut rng, buffer_calls) };
            for _ in 0..1 + rng.below(3) { let i = rng.below(2); m.words[i] ^= 1 << rng.below(usize::BITS as usize); }
        } else {
            let method = if !shape.methods.is_empty() && rng.chance(80) { shape.methods[rng.below(shape.methods.len())] } else { rng.below(256) };
            let version = if rng.chance(90) { major } else { rng.below(256) };
            let field = |rng: &mut Rng| match rng.below(4) { 0 => 0, 1 => rng.below(1 << 16), 2 => 1 << rng.below(48), _ => rng.next() as usize };
            m.words = [method | version << 8 | field(&mut rng) << 16, field(&mut rng)];
            m.cap = cap_kind(&mut rng, buffer_calls);
        }
        if m.cap == Some(CAP_KIND_MEMORY) {
            let (base, donor) = (buffers.pick(&mut rng).to_vec(), buffers.pick(&mut rng).to_vec());
            let payload = if rng.chance(70) { mutate(&mut rng, &base, &donor, shape.copy_max + 8) } else { (0..rng.below(64)).map(|_| rng.byte()).collect() };
            if rng.chance(90) { m.words[0] = (m.words[0] & 0xFFFF) | payload.len() << 16; m.words[1] = 0; }
            // The client's buffer: room for every reply, sometimes too small for the request or the reply.
            let size = if rng.chance(85) { room.max(payload.len()) } else { rng.below(room + 32) };
            m.buffer = payload; m.buffer.resize(size, 0);
        }
        if try_message(&m, &mut words_corpus, &mut buffers, &mut findings) { accepted += 1; }
    }
    (accepted, findings)
}

/// Runs a receiver whose requests are owned values, and checks them against the client's later writes.
macro_rules! owned {
    ($m:ident) => {
        (stringify!($m), idl::$m::VERSION.0 as usize, Box::new(|received: &ipc::Received| {
            match idl::$m::decode(received, ipc::SERVER_SLOT) {
                Ok((request, call)) => { let text = format!("{request:?}"); scribble(); let changed = format!("{request:?}") != text; drop(call); (Ok(text), changed) }
                Err(reject) => (Err(reject), false),
            }
        }) as Box<dyn Fn(&ipc::Received) -> (Result<String, Reject>, bool)>)
    };
}
/// Runs a receiver whose requests borrow its private copy (`REQUEST_MAX`).
macro_rules! borrowed {
    ($m:ident) => {
        (stringify!($m), idl::$m::VERSION.0 as usize, Box::new(|received: &ipc::Received| {
            let mut scratch = Box::new([0u8; idl::$m::REQUEST_MAX]);
            match idl::$m::decode(received, ipc::SERVER_SLOT, &mut scratch) {
                Ok((request, call)) => { let text = format!("{request:?}"); scribble(); let changed = format!("{request:?}") != text; drop(call); (Ok(text), changed) }
                Err(reject) => (Err(reject), false),
            }
        }) as Box<dyn Fn(&ipc::Received) -> (Result<String, Reject>, bool)>)
    };
}

/// The client writes its whole buffer while the server still holds the decoded request.
fn scribble() {
    if let Some((address, length)) = ipc::memory(ipc::SERVER_SLOT) { for i in 0..length { unsafe { let b = (address as *mut u8).add(i); *b = !*b; } } }
}

#[test]
fn every_receiver_survives_fuzzed_messages() {
    let receivers = [
        owned!(audio), owned!(block), borrowed!(blockstore), owned!(display), owned!(gpio), owned!(init), owned!(keyboard),
        borrowed!(keystore), owned!(loader), owned!(log), borrowed!(net), owned!(netpolicy), borrowed!(parse), owned!(rtc),
        owned!(shell), borrowed!(socket), owned!(sysinfo), borrowed!(tls), borrowed!(tpm), owned!(tts), owned!(usb), borrowed!(vfs),
        owned!(video), owned!(voice), owned!(window),
    ];
    let mut names: Vec<String> = receivers.iter().map(|r| r.0.to_string()).collect(); names.sort();
    let mut modules = generated_modules(); modules.sort();
    assert_eq!(names, modules, "every generated interface has a receiver here");
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut failed = Vec::new();
    for (name, major, decode) in &receivers {
        let shape = shape(name);
        let (accepted, findings) = fuzz_receiver(name, *major, &shape, decode.as_ref());
        println!("{name}: {} methods, {} messages, {accepted} accepted, {} findings (seed {:#x})", shape.methods.len(), iterations(), findings.0.len(), seed());
        failed.extend(findings.0);
    }
    panic::set_hook(hook);
    assert!(failed.is_empty(), "receiver findings:\n{}", failed.join("\n"));
}

// --- Types: `Wire::decode` of every generated type ----------------------------------------------------------------

/// Fuzzes one type: whatever it accepts must encode back to the bytes it consumed, within `MAX`.
fn fuzz_type<T: Wire + core::fmt::Debug>(name: &str, findings: &mut Vec<String>) -> usize {
    let mut rng = Rng::new(seed(), name);
    let mut corpus = Corpus(Vec::new());
    let mut accepted = 0;
    let limit = T::MAX + 8;
    let mut encoded = vec![0u8; T::MAX];
    let mut check = |input: &[u8], corpus: &mut Corpus, findings: &mut Vec<String>| -> bool {
        let result = guarded(|| { let mut r = Reader::new(input); let v = T::decode(&mut r); let done = r.done(); (v, done) });
        let (value, done) = match result { Ok(r) => r, Err(p) => { if findings.len() < 20 { findings.push(format!("{name}: panicked ({p}) on [{}]", hex(input))); } return false; } };
        let Some(value) = value else { return false };
        let mut w = Writer::new(&mut encoded);
        let ok = value.encode(&mut w).is_some();
        let n = w.len();
        if !ok || n > T::MAX { if findings.len() < 20 { findings.push(format!("{name}: {value:?} decoded from [{}] does not encode within MAX {}", hex(input), T::MAX)); } return true; }
        // A canonical encoding: what was consumed is exactly what encoding gives back.
        if !input.starts_with(&encoded[..n]) || (done && n != input.len()) {
            if findings.len() < 20 { findings.push(format!("{name}: accepted [{}] as {value:?}, which encodes as [{}]", hex(input), hex(&encoded[..n]))); }
        }
        if done { corpus.add(input); }
        true
    };
    for length in 0..1024usize.min(limit) { if check(&vec![0; length], &mut corpus, findings) { accepted += 1; } }
    for _ in 0..iterations() {
        let (base, donor) = (corpus.pick(&mut rng).to_vec(), corpus.pick(&mut rng).to_vec());
        let input = if rng.chance(75) { mutate(&mut rng, &base, &donor, limit) } else { (0..rng.below(64.min(limit))).map(|_| rng.byte()).collect() };
        if check(&input, &mut corpus, findings) { accepted += 1; }
    }
    accepted
}

macro_rules! types {
    ($($m:ident: $($t:ident)*;)*) => {
        fn fuzz_types(findings: &mut Vec<String>) -> Vec<(String, usize)> {
            let mut done = Vec::new();
            $($( done.push((concat!(stringify!($m), "::", stringify!($t)).to_string(), fuzz_type::<idl::$m::$t>(concat!(stringify!($m), "::", stringify!($t)), findings))); )*)*
            done
        }
        fn listed() -> Vec<String> { vec![$($(concat!(stringify!($m), "::", stringify!($t)).to_string(),)*)*] }
    };
}
types! {
    blockstore: Error Codec Stats Head Collected Kept Pinned Usage Update;
    display: Error Mode;
    gpio: Kind Pull Error Controller Pin;
    init: Error Service;
    keyboard: Layout SwitchKey State;
    keystore: Error Purpose Usage;
    loader: Error Program Needs;
    log: Error Entry State;
    net: Error Info Counters;
    netpolicy: Error Grant;
    parse: Error HttpHead Channel File Manifest ManifestRef Model ModelFile;
    rtc: Error;
    shell: Error;
    socket: Error Protocol Rule Interface Usage Config Datagram;
    sysinfo: Error Task Cpu Memory Range Region Capability EndpointInfo Irq Device Holder AuthorityEntry Sample Load Pool;
    tls: Error Peer;
    tpm: Error Info;
    vfs: Error Entry Volume Report;
    video: Error Camera Frame;
    voice: Action Order;
    window: Error Kind Placement Info;
}

/// libmind/src/idl, from the repository root where the host tests run.
fn idl_dir() -> std::path::PathBuf { std::path::Path::new(file!()).parent().unwrap().join("../libmind/src/idl") }

/// The generated modules, from libmind/src/idl/mod.rs.
fn generated_modules() -> Vec<String> {
    let text = std::fs::read_to_string(idl_dir().join("mod.rs")).expect("libmind/src/idl/mod.rs (run from the repository root)");
    text.lines().filter_map(|l| l.strip_prefix("pub mod ")).map(|m| m.trim_end_matches(';').to_string()).filter(|m| m != "codec" && m != "wire").collect()
}

/// Every `impl Wire for` in the generated modules.
fn generated_types() -> Vec<String> {
    let dir = idl_dir();
    let mut found = Vec::new();
    for m in generated_modules() {
        let text = std::fs::read_to_string(dir.join(format!("{m}.rs"))).unwrap();
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("impl Wire for ") { found.push(format!("{m}::{}", rest.split_whitespace().next().unwrap())); }
        }
    }
    found
}

#[test]
fn every_generated_type_decodes_only_canonical_encodings() {
    let mut listed = listed(); listed.sort();
    let mut generated = generated_types(); generated.sort();
    assert_eq!(listed, generated, "the fuzzer lists every generated type");
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let mut findings = Vec::new();
    let done = fuzz_types(&mut findings);
    panic::set_hook(hook);
    for (name, accepted) in &done { println!("{name}: {} inputs, {accepted} accepted (seed {:#x})", iterations(), seed()); }
    assert!(findings.is_empty(), "type findings:\n{}", findings.join("\n"));
}
