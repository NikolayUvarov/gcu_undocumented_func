#![no_std]
#![no_main]
// memtest: test program of task memory beyond the kernel arena (issue 150).
// `memtest alloc <MiB>` takes MiB of heap in 16 MiB blocks, writes and checks every word, tries one block beyond its
// quota, and frees everything; `memtest hold <MiB>` keeps them until killed. `memtest share <MiB>` fills a block, seals it (a read-only memory object), maps it and
// starts a second memtest that maps the same object; both check the contents. `memtest fill` takes all the heap it can
// get, in ever smaller blocks down to 64 KiB, and keeps it until killed (issue 169: the recovery reserve stops it).
use mind::abi::{BootInfo, CAP_READ};
use mind::idl::loader;
use mind::ipc::{self, Endpoint, Message};
use mind::mem::{Mapping, Pages};

mind::request!(REQUEST_CONSOLE, memory: QUOTA_MIB);

const QUOTA_MIB: u32 = 160;
const MIB: usize = 1024 * 1024;
const BLOCK: usize = 16 * MIB;
const RECEIVED: usize = 9; // the (fixed) slot the child receives the object in

fn word(index: usize) -> u64 { (index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5A5A }

fn fill(memory: &mut [u8], first: usize) {
    for (i, chunk) in memory.chunks_exact_mut(8).enumerate() { chunk.copy_from_slice(&word(first + i).to_le_bytes()); }
}
fn intact(memory: &[u8], first: usize) -> bool {
    memory.chunks_exact(8).enumerate().all(|(i, chunk)| u64::from_le_bytes(chunk.try_into().unwrap()) == word(first + i))
}

fn alloc(mib: usize, hold: bool) {
    let mut blocks: [Option<Pages>; 64] = [const { None }; 64];
    let count = mib.div_ceil(16).min(blocks.len());
    for (n, block) in blocks[..count].iter_mut().enumerate() {
        let Some(mut pages) = Pages::new(BLOCK) else { mind::println!("[MEMTEST] OUT OF MEMORY AFTER {} MiB", n * 16); return };
        fill(pages.as_mut_slice(), n * BLOCK / 8);
        *block = Some(pages);
    }
    let ok = blocks[..count].iter().enumerate().all(|(n, b)| b.as_ref().is_some_and(|p| intact(p.as_slice(), n * BLOCK / 8)));
    mind::println!("[MEMTEST] HELD {} MiB INTACT={}", count * 16, ok);
    if hold { loop { mind::time::sleep(1000); } } // until killed
    // The quota stops the next block (memory beyond it belongs to others).
    let beyond = Pages::new(QUOTA_MIB as usize * MIB);
    mind::println!("[MEMTEST] BEYOND QUOTA: {}", if beyond.is_some() { "GRANTED" } else { "REFUSED" });
    drop(beyond);
    for block in blocks.iter_mut() { block.take(); }
    mind::println!("[MEMTEST] FREED");
}

fn fill_all() {
    let mut blocks: [Option<Pages>; 64] = [const { None }; 64];
    let (mut used, mut total) = (0, 0);
    for size in [BLOCK, 4 * MIB, MIB, 256 * 1024, 64 * 1024] {
        while used < blocks.len() {
            let Some(mut pages) = Pages::new(size) else { break };
            pages.as_mut_slice()[0] = 1;
            blocks[used] = Some(pages); used += 1; total += size;
        }
    }
    mind::println!("[MEMTEST] FILLED {} KiB", total / 1024);
    loop { mind::time::sleep(1000); } // until killed
}

// Whether the contents are the pattern, and their sum, from one word in every 512 bytes (emulation is slow).
fn check(memory: &[u8]) -> (bool, u64) {
    (0..memory.len() / 512).fold((true, 0u64), |(ok, s), n| {
        let (i, at) = (n * 64 + n % 64, (n * 64 + n % 64) * 8);
        let v = u64::from_le_bytes(memory[at..at + 8].try_into().unwrap());
        (ok && v == word(i), s.wrapping_add(v))
    })
}

fn share(mib: usize) {
    let Some(mut pages) = Pages::new(mib * MIB) else { mind::println!("[MEMTEST] OUT OF MEMORY"); return };
    fill(pages.as_mut_slice(), 0);
    // A sealed object: detached from this heap, only a read-only capability left.
    let Ok(object) = pages.detach() else { mind::println!("[MEMTEST] DETACH FAILED"); return };
    let Ok(sealed) = ipc::mint(object, CAP_READ, 0, 0) else { mind::println!("[MEMTEST] MINT FAILED"); return };
    let _ = ipc::drop_cap(object);
    mind::println!("[MEMTEST] SEALED={}", mind::mem::sealed(sealed));
    let mapping = match Mapping::new(sealed) { Ok(m) => m, Err(e) => { mind::println!("[MEMTEST] MAP FAILED {:?}", e); return } };
    let (ok, total) = check(mapping.as_slice());
    mind::println!("[MEMTEST] PARENT MAPPED {} MiB INTACT={} SUM={:x}", mapping.len() / MIB, ok, total);
    // The second task gets this endpoint in its INIT slot and asks for the object.
    let Ok(endpoint) = Endpoint::create() else { mind::println!("[MEMTEST] NO ENDPOINT"); return };
    let started = loader::begin(Endpoint::LOADER, "memtest", "child")
        .and_then(|s| s.map_err(|_| mind::Error::Invalid))
        .and_then(|session| { let client = ipc::mint(endpoint.0, mind::abi::CAP_WRITE | mind::abi::CAP_GRANT, 0, 0)?; loader::grant(Endpoint::LOADER, session, mind::abi::SLOT_INIT as u8, client)?.map_err(|_| mind::Error::Invalid)?; Ok(session) })
        .and_then(|session| loader::commit(Endpoint::LOADER, session)?.map_err(|_| mind::Error::Invalid));
    let Ok(child) = started else { mind::println!("[MEMTEST] CHILD NOT STARTED {:?}", started); return };
    mind::println!("[MEMTEST] CHILD PID={}", child);
    match endpoint.recv_timeout(0, 10_000) {
        Ok(call) if call.is_call => { let _ = ipc::reply(&Message::new(0, 0).with_cap(sealed, CAP_READ)); }
        other => { mind::println!("[MEMTEST] NO CALL FROM THE CHILD {:?}", other.map(|r| r.data)); return }
    }
    // The child's answer: its sum of the contents.
    match endpoint.recv_timeout(0, 120_000) {
        Ok(answer) => {
            let (mib, sealed, ok) = (answer.data[1] >> 2, answer.data[1] & 2 != 0, answer.data[1] & 1 != 0);
            mind::println!("[MEMTEST] CHILD MAPPED {} MiB READ-ONLY SEALED={} INTACT={} SAME SUM={}", mib, sealed, ok, answer.data[0] as u64 == total);
        }
        Err(e) => mind::println!("[MEMTEST] NO ANSWER {:?}", e),
    }
    drop(mapping);
    let _ = ipc::drop_cap(sealed);
    mind::println!("[MEMTEST] SHARE DONE");
}

fn child() {
    let Ok(_) = Endpoint::INIT.call(&Message::new(0, 0), RECEIVED) else { mind::println!("[MEMTEST] CHILD CALL FAILED"); return };
    let mapping = match Mapping::new(RECEIVED) { Ok(m) => m, Err(e) => { mind::println!("[MEMTEST] CHILD MAP FAILED {:?}", e); return } };
    let (ok, total) = check(mapping.as_slice());
    // The parent reports it: a console program's output is gone once it ends.
    let flags = mapping.len() / MIB << 2 | (mind::mem::sealed(RECEIVED) as usize) << 1 | ok as usize;
    let _ = Endpoint::INIT.send(&Message::new(total as usize, flags));
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("memtest — test program of task memory beyond the kernel arena: large heaps and shared read-only objects.\nUsage: memtest alloc <MiB> | memtest hold <MiB> | memtest share <MiB> | memtest fill");
    let mut words = mind::process::args_str().split_whitespace();
    let (command, mib) = (words.next().unwrap_or(""), words.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(16));
    match command {
        "alloc" => alloc(mib, false),
        "hold" => alloc(mib, true),
        "share" => share(mib),
        "fill" => fill_all(),
        "child" => child(),
        _ => mind::println!("USAGE: MEMTEST ALLOC <MiB> | MEMTEST HOLD <MiB> | MEMTEST SHARE <MiB> | MEMTEST FILL"),
    }
}
