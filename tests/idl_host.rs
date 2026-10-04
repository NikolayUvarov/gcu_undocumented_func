//! Host tests of generated MIND IDL bindings: tests/idl/sample.rs (generated from tests/idl/sample.wit: v0.2 types,
//! enums, bytes and capability results) with the real codec and wire modules, and a loopback in place of the kernel:
//! a client call runs the server's `decode` and reply function and returns its reply.
#![allow(dead_code)]
#[path = "../common/abi.rs"]
mod abi;
#[path = "../libmind/src/sys.rs"]
mod sys;

mod dev {
    /// Kind of what a slot holds in the loopback: the server's receive slot holds what the last request carried.
    pub fn cap_info(slot: usize) -> (usize, usize, usize) { (crate::ipc::kind(slot), 0, 0) }
}

mod mem {
    use std::cell::RefCell;
    thread_local! {
        // Capabilities over client buffers: handle -> (address, length).
        pub static SHARED: RefCell<Vec<(usize, usize, usize)>> = RefCell::new(Vec::new());
    }
    pub struct Pages(Vec<u8>);
    impl Pages {
        pub fn new(bytes: usize) -> Option<Self> { Some(Self(vec![0; bytes])) }
        pub fn len(&self) -> usize { self.0.len() }
        pub fn as_slice(&self) -> &[u8] { &self.0 }
        pub fn as_mut_slice(&mut self) -> &mut [u8] { &mut self.0 }
        pub fn share(&self) -> crate::sys::Result<usize> {
            let handle = 0x100 + SHARED.with(|s| s.borrow().len());
            SHARED.with(|s| s.borrow_mut().push((handle, self.0.as_ptr() as usize, self.0.len())));
            Ok(handle)
        }
    }
    /// The server's view of the buffer that came with the request (the same memory).
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
    use std::cell::RefCell;
    use std::collections::HashMap;
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
    thread_local! {
        pub static SERVER: RefCell<Option<fn(&Received)>> = RefCell::new(None);
        pub static REPLY: RefCell<Option<Message>> = RefCell::new(None);
        // Slot -> (kind, memory) on either side of the loopback; dropped slots.
        pub static SLOTS: RefCell<HashMap<usize, (usize, Option<(usize, usize)>)>> = RefCell::new(HashMap::new());
        pub static DROPPED: RefCell<Vec<usize>> = RefCell::new(Vec::new());
    }
    pub const SERVER_SLOT: usize = 9;
    pub fn kind(slot: usize) -> usize { SLOTS.with(|s| s.borrow().get(&slot).map_or(CAP_KIND_NONE, |e| e.0)) }
    pub fn memory(slot: usize) -> Option<(usize, usize)> { SLOTS.with(|s| s.borrow().get(&slot).and_then(|e| e.1)) }
    pub fn request(data: [usize; 2], cap: Option<usize>) -> Received {
        SLOTS.with(|s| { let mut s = s.borrow_mut(); s.remove(&SERVER_SLOT); if let Some(kind) = cap { s.insert(SERVER_SLOT, (kind, None)); } });
        Received { data, sender: 2, badge: 0, cap_received: cap.is_some(), is_call: true, irq: None }
    }
    impl Endpoint {
        pub fn call(&self, message: &Message, receive: usize) -> crate::sys::Result<Received> {
            let shared = crate::mem::SHARED.with(|s| s.borrow().iter().find(|e| e.0 == message.cap).map(|e| (e.1, e.2)));
            let request = request(message.data, (message.cap != 0).then_some(if shared.is_some() { CAP_KIND_MEMORY } else { CAP_KIND_ENDPOINT }));
            if let Some(memory) = shared { SLOTS.with(|s| s.borrow_mut().get_mut(&SERVER_SLOT).unwrap().1 = Some(memory)); }
            let server = SERVER.with(|s| s.borrow().expect("a server"));
            server(&request);
            let reply = REPLY.with(|r| r.borrow_mut().take()).expect("the server replied");
            let received = reply.cap != 0 && receive != 0;
            if received { SLOTS.with(|s| s.borrow_mut().insert(receive, (CAP_KIND_ENDPOINT, None))); }
            Ok(Received { data: reply.data, sender: 1, badge: 0, cap_received: received, is_call: false, irq: None })
        }
    }
    pub fn reply(message: &Message) -> crate::sys::Result<()> { REPLY.with(|r| *r.borrow_mut() = Some(*message)); Ok(()) }
    pub fn save_reply() -> crate::sys::Result<usize> { Ok(30) }
    pub fn reply_saved(_slot: usize, message: &Message) -> crate::sys::Result<()> { reply(message) }
    pub fn drop_cap(slot: usize) -> crate::sys::Result<()> { DROPPED.with(|d| d.borrow_mut().push(slot)); SLOTS.with(|s| s.borrow_mut().remove(&slot)); Ok(()) }
    pub fn revoke(_slot: usize) -> crate::sys::Result<usize> { Ok(1) }
}

mod idl {
    #[path = "../../libmind/src/idl/codec.rs"]
    pub mod codec;
    #[path = "../../libmind/src/idl/wire.rs"]
    pub mod wire;
    #[path = "../../tests/idl/sample.rs"]
    pub mod sample;
}

use idl::codec::{Text, Wire, Writer};
use idl::sample::{self, Entry, Error as E, Kind, Pair, Request};
use idl::wire::{self, Reject};

fn entry(name: &str, size: u64, hidden: bool, kind: Kind) -> Entry { Entry { name: Text::new(name).unwrap(), size, hidden, kind } }

fn entries() -> [Entry; 3] {
    [entry("kernel.elf", 123_456, false, Kind::File), entry("EFI", 0, false, Kind::Directory), entry("Документы", 7, true, Kind::Directory)]
}

// The endpoint capability `open` hands out.
const HANDED: usize = 0x42;

fn server(request: &ipc::Received) {
    let mut scratch = [0u8; sample::REQUEST_MAX];
    let _ = match sample::decode(request, ipc::SERVER_SLOT, &mut scratch) {
        Ok((Request::Ping { x, flag }, call)) => sample::reply_ping(call, x + flag as u32),
        Ok((Request::Find { dir, name }, call)) => sample::reply_find(call, if name.as_str() == "Документы" { Ok(dir * 10) } else { Err(E::NotFound) }),
        Ok((Request::List { start }, call)) => sample::reply_list(call, &entries()[(start as usize).min(3)..]),
        Ok((Request::Put { items, note }, call)) => {
            let sum = items.as_slice().iter().map(|&i| i as u32).sum::<u32>() + note.iter().map(|&b| b as u32).sum::<u32>();
            sample::reply_put(call, if sum == 0 { Err(E::TooBig) } else { Ok(()) })
        }
        Ok((Request::Info { of }, call)) => sample::reply_info(call, &Pair { tag: of as u8, entry: entries()[2] }),
        Ok((Request::Mode { k }, call)) => sample::reply_mode(call, (k != Kind::Device).then_some(if k == Kind::File { Kind::Directory } else { Kind::File })),
        Ok((Request::Fetch { length }, call)) => {
            let data: Vec<u8> = (0..length as u8).collect();
            sample::reply_fetch(call, if length > 64 { Err(E::TooBig) } else { Ok(&data) })
        }
        Ok((Request::Open { id }, call)) => sample::reply_open(call, if id == 1 { Ok(HANDED) } else { Err(E::Exists) }),
        Ok((Request::Sys { k, fail }, call)) => sample::reply_sys(call, if fail { Err(sys::Error::Rights) } else { Ok(k) }),
        Err(reason) => wire::reject(reason),
    };
}

fn serve(f: fn(&ipc::Received)) -> ipc::Endpoint { ipc::SERVER.with(|s| *s.borrow_mut() = Some(f)); ipc::Endpoint(3) }

#[test]
fn word_calls_with_enums_options_and_system_errors() {
    let endpoint = serve(server);
    assert_eq!(sample::ping(endpoint, 41, true), Ok(42));
    assert_eq!(sample::mode(endpoint, Kind::File), Ok(Some(Kind::Directory)));
    assert_eq!(sample::mode(endpoint, Kind::Device), Ok(None));
    assert_eq!(sample::sys(endpoint, Kind::Device, false), Ok(Kind::Device));
    assert_eq!(sample::sys(endpoint, Kind::File, true), Err(sys::Error::Rights));
}

#[test]
fn strings_records_lists_and_enum_errors() {
    let endpoint = serve(server);
    assert_eq!(sample::find(endpoint, 4, "Документы"), Ok(Ok(40)));
    assert_eq!(sample::find(endpoint, 4, "nope"), Ok(Err(E::NotFound)));
    assert_eq!(sample::find(endpoint, 4, &"x".repeat(65)), Err(sys::Error::Invalid), "over the declared limit");
    let list = sample::list(endpoint, 0).unwrap();
    assert_eq!(list.as_slice().iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["kernel.elf", "EFI", "Документы"]);
    assert_eq!(list.as_slice()[2].kind, Kind::Directory);
    assert!(list.as_slice()[2].hidden);
    let pair = sample::info(endpoint, Kind::Device).unwrap();
    assert_eq!((pair.tag, pair.entry.name.as_str()), (2, "Документы"));
}

#[test]
fn bytes_in_both_directions() {
    let endpoint = serve(server);
    assert_eq!(sample::put(endpoint, &[1, 2, 3], b"note"), Ok(Ok(())));
    assert_eq!(sample::put(endpoint, &[], b""), Ok(Err(E::TooBig)));
    assert_eq!(sample::put(endpoint, &[1], &[0u8; 11]), Err(sys::Error::Invalid), "more bytes than declared");
    let mut out = [0u8; 64];
    assert_eq!(sample::fetch(endpoint, 5, &mut out), Ok(Ok(5)));
    assert_eq!(&out[..5], &[0, 1, 2, 3, 4]);
    assert_eq!(sample::fetch(endpoint, 65, &mut out), Ok(Err(E::TooBig)));
    let mut small = [0u8; 4];
    assert_eq!(sample::fetch(endpoint, 5, &mut small), Err(sys::Error::Invalid), "the reply does not fit the caller's slice");
}

#[test]
fn capability_results() {
    let endpoint = serve(server);
    assert_eq!(sample::open(endpoint, 1, 11), Ok(Ok(())));
    assert_eq!(ipc::kind(11), abi::CAP_KIND_ENDPOINT, "the capability landed in the receive slot");
    assert_eq!(sample::open(endpoint, 2, 12), Ok(Err(E::Exists)));
    assert_eq!(ipc::kind(12), abi::CAP_KIND_NONE);
    // A server that sends a capability with an error: the client drops it.
    let endpoint = serve(|_| { let _ = ipc::reply(&ipc::Message::new(wire::STATUS_ERROR, 1).with_cap(HANDED, 0xFF)); });
    assert_eq!(sample::open(endpoint, 1, 13), Ok(Err(E::Exists)));
    assert!(ipc::DROPPED.with(|d| d.borrow().contains(&13)));
    // Success without a capability is malformed.
    let endpoint = serve(|_| { let _ = ipc::reply(&ipc::Message::new(0, 0)); });
    assert_eq!(sample::open(endpoint, 1, 14), Err(sys::Error::Invalid));
}

#[test]
fn clients_reject_malformed_replies() {
    // An error case the interface does not define, a system error code in place of a case, an unknown enum value.
    let endpoint = serve(|_| { let _ = ipc::reply(&ipc::Message::new(wire::STATUS_ERROR, 9)); });
    assert_eq!(sample::find(endpoint, 1, "a"), Err(sys::Error::Invalid));
    let endpoint = serve(|_| { let _ = ipc::reply(&ipc::Message::new(wire::STATUS_ERROR, abi::ERR_NO_MEMORY)); });
    assert_eq!(sample::find(endpoint, 1, "a"), Err(sys::Error::NoMemory));
    let endpoint = serve(|_| { let _ = ipc::reply(&ipc::Message::new(7 << 16, 0)); });
    assert_eq!(sample::mode(endpoint, Kind::File), Err(sys::Error::Invalid));
    // A reply that claims more bytes than declared.
    let endpoint = serve(|_| { let _ = ipc::reply(&ipc::Message::new(5000 << 16, 0)); });
    assert_eq!(sample::list(endpoint, 0).map(|l| l.len()), Err(sys::Error::Invalid));
}

#[test]
fn servers_reject_malformed_requests() {
    let mut scratch = [0u8; sample::REQUEST_MAX];
    // An unknown enum value in a word call; a buffer call without its buffer; an unknown method.
    let request = ipc::request([6 | 2 << 8 | 7 << 16, 0], None);
    assert_eq!(sample::decode(&request, ipc::SERVER_SLOT, &mut scratch).err(), Some(Reject::Invalid));
    let request = ipc::request([2 | 2 << 8, 0], None);
    assert_eq!(sample::decode(&request, ipc::SERVER_SLOT, &mut scratch).err(), Some(Reject::Invalid));
    let request = ipc::request([99 | 2 << 8, 0], None);
    assert_eq!(sample::decode(&request, ipc::SERVER_SLOT, &mut scratch).err(), Some(Reject::Invalid));
    let request = ipc::request([1 | 1 << 8, 0], None);
    assert_eq!(sample::decode(&request, ipc::SERVER_SLOT, &mut scratch).err(), Some(Reject::Version));
    // Buffer payloads: an enum out of range, bytes longer than declared, trailing bytes.
    let check = |payload: &[u8], method: usize| {
        let mut buffer = mem::Pages::new(4096).unwrap();
        buffer.as_mut_slice()[..payload.len()].copy_from_slice(payload);
        let handle = buffer.share().unwrap();
        let memory = mem::SHARED.with(|s| s.borrow().iter().find(|e| e.0 == handle).map(|e| (e.1, e.2)));
        let request = ipc::request([method | 2 << 8 | payload.len() << 16, 0], Some(abi::CAP_KIND_MEMORY));
        ipc::SLOTS.with(|s| s.borrow_mut().get_mut(&ipc::SERVER_SLOT).unwrap().1 = memory);
        let mut scratch = [0u8; sample::REQUEST_MAX];
        sample::decode(&request, ipc::SERVER_SLOT, &mut scratch).map(|(request, _)| format!("{request:?}"))
    };
    assert_eq!(check(&[1], 5), Ok("Info { of: Directory }".to_string()));
    assert_eq!(check(&[3], 5), Err(Reject::Invalid), "kind 3 does not exist");
    let mut put = vec![1, 0, 7, 0, 2, 0, b'h', b'i'];
    assert_eq!(check(&put, 4), Ok("Put { items: [7], note: [104, 105] }".to_string()));
    put.push(0);
    assert_eq!(check(&put, 4), Err(Reject::Invalid), "trailing bytes");
    let mut long = vec![0, 0, 11, 0];
    long.extend([0u8; 11]);
    assert_eq!(check(&long, 4), Err(Reject::Invalid), "bytes<10> holds at most 10 bytes");
    // The encodings: bytes<N> as list<u8, N>, an enum as one byte.
    let mut out = [0u8; 16];
    let mut w = Writer::new(&mut out);
    idl::codec::encode_bytes::<4>(b"ab", &mut w).unwrap();
    Kind::Device.encode(&mut w).unwrap();
    let n = w.len();
    assert_eq!(&out[..n], &[2, 0, b'a', b'b', 2]);
    assert!(idl::codec::encode_bytes::<1>(b"ab", &mut Writer::new(&mut out)).is_none());
}
