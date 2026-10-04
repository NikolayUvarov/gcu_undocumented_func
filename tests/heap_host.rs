//! Host tests of the program heap (libmind/src/heap.rs) over a simulated page source.
#[path = "../libmind/src/heap.rs"]
mod heap;
use heap::{Heap, PageSource, ARENA_BYTES, MAX_ARENAS, PAGE};
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::collections::HashMap;

#[derive(Default)]
struct Host { blocks: HashMap<usize, usize>, limit: usize }
impl PageSource for Host {
    fn alloc(&mut self, bytes: usize) -> usize {
        assert!(bytes % PAGE == 0 && bytes > 0);
        if self.blocks.len() >= self.limit { return 0; }
        let address = unsafe { alloc_zeroed(Layout::from_size_align(bytes, PAGE).unwrap()) } as usize;
        self.blocks.insert(address, bytes);
        address
    }
    fn free(&mut self, address: usize, bytes: usize) {
        assert_eq!(self.blocks.remove(&address), Some(bytes), "free of an unknown block");
        unsafe { dealloc(address as *mut u8, Layout::from_size_align(bytes, PAGE).unwrap()) };
    }
}

struct Rng(u64);
impl Rng { fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 } }

#[test]
fn random_workload_is_aligned_disjoint_and_intact() {
    let mut heap = Heap::new(Host { limit: 32, ..Default::default() });
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut live: Vec<(usize, Layout, u8)> = Vec::new();
    for step in 0..20_000 {
        if live.len() < 400 && rng.next() % 3 != 0 {
            let size = match rng.next() % 10 { 0 => 1 + rng.next() as usize % 300_000, 1..=2 => 1 + rng.next() as usize % 20_000, _ => 1 + rng.next() as usize % 2048 };
            let align = 1 << (rng.next() % 13);
            let layout = Layout::from_size_align(size, align).unwrap();
            let pointer = unsafe { heap.alloc(layout) } as usize;
            assert_ne!(pointer, 0, "allocation {step} of {size} failed");
            assert_eq!(pointer % align, 0, "misaligned");
            for &(other, other_layout, _) in &live { assert!(pointer + size <= other || other + other_layout.size() <= pointer, "overlap"); }
            let tag = step as u8;
            unsafe { std::ptr::write_bytes(pointer as *mut u8, tag, size) };
            live.push((pointer, layout, tag));
        } else if !live.is_empty() {
            let (pointer, layout, tag) = live.swap_remove(rng.next() as usize % live.len());
            let bytes = unsafe { std::slice::from_raw_parts(pointer as *const u8, layout.size()) };
            assert!(bytes.iter().all(|&b| b == tag), "object was overwritten");
            unsafe { heap.dealloc(pointer as *mut u8, layout) };
        }
    }
    for (pointer, layout, _) in live.drain(..) { unsafe { heap.dealloc(pointer as *mut u8, layout) }; }
    let stats = heap.stats();
    assert_eq!(stats.large_blocks, 0);
    assert!(stats.arenas <= MAX_ARENAS);
}

#[test]
fn small_objects_reuse_freed_memory_and_use_one_arena() {
    let mut heap = Heap::new(Host { limit: 32, ..Default::default() });
    let layout = Layout::from_size_align(24, 8).unwrap();
    let first: Vec<usize> = (0..1000).map(|_| unsafe { heap.alloc(layout) } as usize).collect();
    for &p in &first { unsafe { heap.dealloc(p as *mut u8, layout) }; }
    let second: Vec<usize> = (0..1000).map(|_| unsafe { heap.alloc(layout) } as usize).collect();
    let mut a = first.clone(); a.sort(); let mut b = second.clone(); b.sort();
    assert_eq!(a, b, "freed objects are reused");
    assert_eq!(heap.stats().arenas, 1);
}

#[test]
fn exhaustion_returns_null_and_recovers() {
    let mut heap = Heap::new(Host { limit: 2, ..Default::default() });
    let medium = Layout::from_size_align(ARENA_BYTES / 2, 8).unwrap(); // too big for medium: a large block
    let a = unsafe { heap.alloc(medium) };
    let b = unsafe { heap.alloc(medium) };
    assert!(!a.is_null() && !b.is_null());
    assert!(unsafe { heap.alloc(medium) }.is_null(), "past the block limit allocation fails");
    unsafe { heap.dealloc(a, medium) };
    assert!(!unsafe { heap.alloc(medium) }.is_null(), "a freed block can be used again");
    assert!(unsafe { heap.alloc(Layout::from_size_align(16, 8192).unwrap()) }.is_null(), "alignment above a page is refused");
}

#[test]
fn medium_runs_coalesce_after_free() {
    let mut heap = Heap::new(Host { limit: 32, ..Default::default() });
    let quarter = Layout::from_size_align(ARENA_BYTES / 4, PAGE).unwrap();
    let parts: Vec<*mut u8> = (0..4).map(|_| unsafe { heap.alloc(quarter) }).collect();
    assert_eq!(heap.stats().arenas, 1);
    for &p in &parts { unsafe { heap.dealloc(p, quarter) }; }
    assert_eq!(heap.stats().pages_used, 0);
    let half = Layout::from_size_align(200 * 1024, PAGE).unwrap();
    let x = unsafe { heap.alloc(half) };
    assert_eq!(x, parts[0], "the run starts at the first free page again");
    assert_eq!(heap.stats().arenas, 1);
}
