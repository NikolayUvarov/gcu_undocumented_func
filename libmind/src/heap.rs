//! Program heap (cargo feature `alloc`): a `GlobalAlloc` over page blocks from the kernel (`ALLOC`/`FREE`).
//! Small objects (up to 2 KiB) come from power-of-two size classes carved out of single pages, medium objects (up to
//! 256 KiB) are runs of pages inside 1 MiB arenas, large objects get their own page block. Arenas are added on demand,
//! so small objects use a few of the task's 32 blocks. This file has no dependencies: the host tests build it too.
use core::alloc::Layout;

pub const PAGE: usize = 4096;
pub const ARENA_BYTES: usize = 1 << 20;
const ARENA_PAGES: usize = ARENA_BYTES / PAGE;
const WORDS: usize = ARENA_PAGES / 64;
/// At most 12 MiB of arenas: the rest of the 16 MiB / 32-block budget stays for large objects and explicit `Pages`.
pub const MAX_ARENAS: usize = 12;
pub const SMALL_MAX: usize = 2048;
pub const MEDIUM_MAX: usize = 256 * 1024;
const MIN_CLASS: usize = 16;
const CLASSES: usize = 8; // 16, 32, ..., 2048

/// Where page blocks come from: the kernel in a program, the host allocator in tests. Blocks are page-aligned.
pub trait PageSource {
    /// Page-aligned block of `bytes` (a multiple of the page size), or 0.
    fn alloc(&mut self, bytes: usize) -> usize;
    fn free(&mut self, address: usize, bytes: usize);
}

#[derive(Clone, Copy)]
struct Arena { base: usize, used: [u64; WORDS] }

impl Arena {
    fn is_used(&self, page: usize) -> bool { self.used[page / 64] & 1 << (page % 64) != 0 }
    fn mark(&mut self, first: usize, count: usize, used: bool) {
        for page in first..first + count { if used { self.used[page / 64] |= 1 << (page % 64); } else { self.used[page / 64] &= !(1 << (page % 64)); } }
    }
    // First fit for `count` contiguous free pages.
    fn find(&self, count: usize) -> Option<usize> {
        let mut run = 0;
        for page in 0..ARENA_PAGES {
            if self.is_used(page) { run = 0; continue; }
            run += 1;
            if run == count { return Some(page + 1 - count); }
        }
        None
    }
}

/// Counters for diagnostics (`memmap` shows them for its own process).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats { pub arenas: usize, pub large_blocks: usize, pub large_bytes: usize, pub pages_used: usize }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind { Small(usize), Medium(usize), Large(usize), Unsupported }

// The same layout always takes the same path, so `dealloc` needs no header.
fn kind(layout: Layout) -> Kind {
    let size = layout.size().max(1);
    if layout.align() > PAGE { return Kind::Unsupported; }
    if size.max(layout.align()) <= SMALL_MAX {
        let class = size.max(layout.align()).max(MIN_CLASS).next_power_of_two();
        return Kind::Small(class.trailing_zeros() as usize - MIN_CLASS.trailing_zeros() as usize);
    }
    let pages = size.div_ceil(PAGE);
    if size <= MEDIUM_MAX { Kind::Medium(pages) } else { Kind::Large(pages * PAGE) }
}

pub struct Heap<S: PageSource> { source: S, arenas: [Option<Arena>; MAX_ARENAS], free: [usize; CLASSES], stats: Stats }

impl<S: PageSource> Heap<S> {
    pub const fn new(source: S) -> Self { Self { source, arenas: [None; MAX_ARENAS], free: [0; CLASSES], stats: Stats { arenas: 0, large_blocks: 0, large_bytes: 0, pages_used: 0 } } }
    pub fn stats(&self) -> Stats { self.stats }

    fn pages(&mut self, count: usize) -> usize {
        for arena in self.arenas.iter_mut().flatten() {
            if let Some(first) = arena.find(count) { arena.mark(first, count, true); self.stats.pages_used += count; return arena.base + first * PAGE; }
        }
        let Some(slot) = self.arenas.iter().position(Option::is_none) else { return 0 };
        let base = self.source.alloc(ARENA_BYTES);
        if base == 0 { return 0; }
        let mut arena = Arena { base, used: [0; WORDS] };
        arena.mark(0, count, true);
        self.arenas[slot] = Some(arena); self.stats.arenas += 1; self.stats.pages_used += count;
        base
    }

    fn release_pages(&mut self, address: usize, count: usize) {
        if let Some(arena) = self.arenas.iter_mut().flatten().find(|a| address >= a.base && address < a.base + ARENA_BYTES) {
            arena.mark((address - arena.base) / PAGE, count, false); self.stats.pages_used -= count;
        }
    }

    /// # Safety
    /// Same contract as `GlobalAlloc::alloc`.
    pub unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        match kind(layout) {
            Kind::Small(class) => {
                if self.free[class] == 0 {
                    // Refill: split one page into objects of this class.
                    let page = self.pages(1);
                    if page == 0 { return core::ptr::null_mut(); }
                    let size = MIN_CLASS << class;
                    for object in (page..page + PAGE).step_by(size).rev() { (object as *mut usize).write(self.free[class]); self.free[class] = object; }
                }
                let object = self.free[class];
                self.free[class] = (object as *const usize).read();
                object as *mut u8
            }
            Kind::Medium(pages) => self.pages(pages) as *mut u8,
            Kind::Large(bytes) => {
                let block = self.source.alloc(bytes);
                if block != 0 { self.stats.large_blocks += 1; self.stats.large_bytes += bytes; }
                block as *mut u8
            }
            Kind::Unsupported => core::ptr::null_mut(),
        }
    }

    /// # Safety
    /// Same contract as `GlobalAlloc::dealloc`.
    pub unsafe fn dealloc(&mut self, pointer: *mut u8, layout: Layout) {
        let address = pointer as usize;
        match kind(layout) {
            Kind::Small(class) => { (address as *mut usize).write(self.free[class]); self.free[class] = address; }
            Kind::Medium(pages) => self.release_pages(address, pages),
            Kind::Large(bytes) => { self.source.free(address, bytes); self.stats.large_blocks -= 1; self.stats.large_bytes -= bytes; }
            Kind::Unsupported => {}
        }
    }
}
