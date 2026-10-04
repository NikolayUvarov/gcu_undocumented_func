//! Host tests of MIND IDL v0.2 bindings: tests/idl/sample.rs (generated from tests/idl/sample.wit) with the real wire
//! module, and a loopback in place of the kernel: a client call runs the server's dispatch and returns its reply.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/sys.rs"]
mod sys;
mod dev { pub fn cap_info(_slot: usize) -> (usize, usize, usize) { (crate::abi::CAP_KIND_MEMORY, 7, 4096) } }
mod mem {
    pub struct Pages(Vec<u8>);
    impl Pages {
        pub fn new(bytes: usize) -> Option<Self> { Some(Self(vec![0; bytes])) }
        pub fn share(&self) -> crate::sys::Result<usize> { Ok(7) }
        pub fn as_mut_slice(&mut self) -> &mut [u8] { &mut self.0 }
    }
}
mod ipc {
    use std::cell::RefCell;
    #[derive(Clone, Copy, Debug)]
    pub struct Message { pub data: [usize; 2], pub cap: usize }
    impl Message {
        pub fn new(a: usize, b: usize) -> Self { Self { data: [a, b], cap: 0 } }
        pub fn with_cap(mut self, cap: usize, _rights: u8) -> Self { self.cap = cap; self }
        pub fn with_cap_moved(self, cap: usize, rights: u8) -> Self { self.with_cap(cap, rights) }
    }
    #[derive(Clone, Copy, Debug)]
    pub struct Received { pub data: [usize; 2], pub sender: u64, pub cap_received: bool, pub is_call: bool, pub irq: Option<usize> }
    #[derive(Clone, Copy)]
    pub struct Endpoint(pub usize);
    thread_local! {
        pub static SERVER: RefCell<Option<fn(&Received)>> = RefCell::new(None);
        pub static REPLY: RefCell<Option<[usize; 2]>> = RefCell::new(None);
        // The client's buffer as the server "maps" it: copied in before dispatch and back after it.
        pub static CLIENT: RefCell<(*mut u8, usize)> = RefCell::new((std::ptr::null_mut(), 0));
        pub static MAPPED: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    }
    impl Endpoint {
        pub fn call(&self, message: &Message, _receive: usize) -> crate::sys::Result<Received> {
            let (ptr, len) = CLIENT.with(|c| *c.borrow());
            if message.cap != 0 { MAPPED.with(|m| *m.borrow_mut() = unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec()); }
            let request = Received { data: message.data, sender: 2, cap_received: message.cap != 0, is_call: true, irq: None };
            let server = SERVER.with(|s| s.borrow().unwrap());
            server(&request);
            if message.cap != 0 { MAPPED.with(|m| unsafe { std::slice::from_raw_parts_mut(ptr, len) }.copy_from_slice(&m.borrow())); }
            let data = REPLY.with(|r| r.borrow_mut().take()).expect("the server replied");
            Ok(Received { data, sender: 1, cap_received: false, is_call: false, irq: None })
        }
    }
    pub fn reply(message: &Message) -> crate::sys::Result<()> { REPLY.with(|r| *r.borrow_mut() = Some(message.data)); Ok(()) }
    pub fn drop_cap(_slot: usize) -> crate::sys::Result<()> { Ok(()) }
}
mod idl {
    #[path = "../../libmind/src/idl/wire.rs"]
    pub mod wire;
    #[path = "../../tests/idl/sample.rs"]
    pub mod sample;
}
use idl::sample::{self, Entry, Error as E, Kind, Pair, Request};
use idl::wire::{self, Buffer, Reject, Writer};

const ENTRIES: [Entry; 3] = [
    Entry { name: "kernel.elf", size: 123_456, hidden: false, kind: Kind::File },
    Entry { name: "EFI", size: 0, hidden: false, kind: Kind::Directory },
    Entry { name: "Документы", size: 7, hidden: true, kind: Kind::Directory },
];

fn mapped<R>(f: impl FnOnce(&mut [u8]) -> R) -> R { ipc::MAPPED.with(|m| f(&mut m.borrow_mut())) }

fn server(request: &ipc::Received) {
    let _ = match sample::decode(request, 9) {
        Ok(Request::Ping { x, flag }) => sample::reply_ping(x + flag as u32),
        Ok(Request::Find { payload, dir, buffer: _ }) => match mapped(|b| sample::args_find(b, payload).map(|name| name == "Документы")) {
            Ok(true) => sample::reply_find(Ok(dir * 10)),
            Ok(false) => sample::reply_find(Err(E::NotFound)),
            Err(reason) => wire::reject(reason),
        },
        Ok(Request::List { start, .. }) => mapped(|b| sample::reply_list(b, &ENTRIES[(start as usize).min(3)..])),
        Ok(Request::Put { payload, .. }) => match mapped(|b| sample::args_put(b, payload).map(|(items, note)| items.iter().map(u32::from).sum::<u32>() + note.len() as u32)) {
            Ok(sum) if sum == 0 => sample::reply_put(Err(E::TooBig)),
            Ok(_) => sample::reply_put(Ok(())),
            Err(reason) => wire::reject(reason),
        },
        Ok(Request::Info { of, .. }) => mapped(|b| sample::reply_info(b, Pair { tag: of as u8, entry: ENTRIES[2] })),
        Ok(Request::Mode { k }) => sample::reply_mode((k != Kind::Device).then_some(k as u8 + 40)),
        Err(reason) => wire::reject(reason),
    };
}

fn with_buffer<R>(size: usize, f: impl FnOnce(Buffer) -> R) -> R {
    ipc::SERVER.with(|s| *s.borrow_mut() = Some(server));
    let mut bytes = vec![0u8; size];
    ipc::CLIENT.with(|c| *c.borrow_mut() = (bytes.as_mut_ptr(), bytes.len()));
    f(Buffer { cap: 7, bytes: &mut bytes })
}

#[test]
fn scalar_calls_and_options() {
    ipc::SERVER.with(|s| *s.borrow_mut() = Some(server));
    let endpoint = ipc::Endpoint(3);
    assert_eq!(sample::ping(endpoint, 41, true), Ok(42));
    assert_eq!(sample::mode(endpoint, Kind::Directory), Ok(Some(41)));
    assert_eq!(sample::mode(endpoint, Kind::Device), Ok(None));
}

#[test]
fn strings_and_fallible_results() {
    let endpoint = ipc::Endpoint(3);
    assert_eq!(with_buffer(256, |b| sample::find(endpoint, 4, b, "Документы")), Ok(Ok(40)));
    assert_eq!(with_buffer(256, |b| sample::find(endpoint, 4, b, "nope")), Ok(Err(E::NotFound)));
    assert_eq!(with_buffer(256, |b| sample::find(endpoint, 4, b, &"x".repeat(65))), Err(sys::Error::Invalid), "over the declared limit");
}

#[test]
fn lists_and_records_come_back_decoded() {
    let endpoint = ipc::Endpoint(3);
    let names: Vec<String> = with_buffer(512, |b| sample::list(endpoint, b, 0).map(|list| list.iter().map(|e| e.name.to_string()).collect())).unwrap();
    assert_eq!(names, ["kernel.elf", "EFI", "Документы"]);
    let tail = with_buffer(512, |b| sample::list(endpoint, b, 2).map(|list| (list.len(), list.iter().next().map(|e| (e.size, e.hidden, e.kind))))).unwrap();
    assert_eq!(tail, (1, Some((7, true, Kind::Directory))));
    let info = with_buffer(128, |b| sample::info(endpoint, b, Kind::Device).map(|p| (p.tag, p.entry.name.to_string()))).unwrap();
    assert_eq!(info, (2, "Документы".to_string()));
    assert_eq!(with_buffer(20, |b| sample::list(endpoint, b, 0).map(|l| l.len())), Err(sys::Error::Other(abi::ERR_LIMIT)), "result too big for the buffer");
    assert_eq!(with_buffer(256, |b| sample::put(endpoint, b, &[1, 2, 3], b"note")), Ok(Ok(())));
    assert_eq!(with_buffer(256, |b| sample::put(endpoint, b, &[], b"")), Ok(Err(E::TooBig)));
}

#[test]
fn receiver_rejects_malformed_payloads() {
    let encode = |f: &dyn Fn(&mut Writer) -> sys::Result<()>| { let mut buf = vec![0u8; 64]; let mut w = Writer::new(&mut buf); f(&mut w).unwrap(); let n = w.len(); buf.truncate(n); buf };
    let good = encode(&|w| { w.list(&[1u16, 2], 8)?; w.bytes(b"ab", 10) });
    assert!(sample::args_put(&good, good.len() as u32).is_ok());
    assert_eq!(sample::args_put(&good, good.len() as u32 + 1).err(), Some(Reject::Invalid), "payload longer than the buffer");
    let mut trailing = good.clone(); trailing.push(0);
    assert_eq!(sample::args_put(&trailing, trailing.len() as u32).err(), Some(Reject::Invalid), "trailing bytes");
    assert_eq!(sample::args_put(&good, good.len() as u32 - 1).err(), Some(Reject::Invalid), "truncated");
    let too_many = encode(&|w| { w.u32(9)?; for _ in 0..9 { w.u16(1)?; } w.bytes(b"", 10) });
    assert_eq!(sample::args_put(&too_many, too_many.len() as u32).err(), Some(Reject::Invalid), "more list items than declared");
    let long_bytes = encode(&|w| { w.list(&[1u16], 8)?; w.u32(11)?; for _ in 0..11 { w.u8(0)?; } Ok(()) });
    assert_eq!(sample::args_put(&long_bytes, long_bytes.len() as u32).err(), Some(Reject::Invalid));
    let bad_utf8 = encode(&|w| { w.u16(2)?; w.u8(0xD0)?; w.u8(0x41) });
    assert_eq!(sample::args_find(&bad_utf8, bad_utf8.len() as u32).err(), Some(Reject::Invalid), "strings must be UTF-8");
    let mut r = wire::Reader::new(&[2]);
    assert_eq!(r.bool(), None, "bool is 0 or 1");
    // A request with an unknown enum value in the words is rejected before any handler runs.
    let request = ipc::Received { data: [6 | 2 << 8 | 7 << 16, 0], sender: 1, cap_received: false, is_call: true, irq: None };
    assert_eq!(sample::decode(&request, 9).err(), Some(Reject::Invalid));
    let request = ipc::Received { data: [2 | 2 << 8, 0], sender: 1, cap_received: false, is_call: true, irq: None };
    assert_eq!(sample::decode(&request, 9).err(), Some(Reject::Invalid), "a bulk call without its buffer");
}

#[test]
fn client_rejects_a_reply_that_overstates_its_length() {
    ipc::SERVER.with(|s| *s.borrow_mut() = Some(|_| { let _ = wire::reply([5000 << 16, 0]); }));
    let mut bytes = vec![0u8; 64];
    ipc::CLIENT.with(|c| *c.borrow_mut() = (bytes.as_mut_ptr(), bytes.len()));
    assert_eq!(sample::list(ipc::Endpoint(3), Buffer { cap: 7, bytes: &mut bytes }, 0).map(|l| l.len()), Err(sys::Error::Invalid));
}
