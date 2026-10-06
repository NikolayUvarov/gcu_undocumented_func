// Physical frames for task memory (issue 150): the free RAM of the firmware map outside the kernel arena. Heap blocks,
// memory objects, screens, images and stacks come from here; kernel structures stay in the arena.
use crate::abi::StatPhys;
use core::alloc::Layout;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};
use linked_list_allocator::Heap;

const RANGES: usize = 16;
const CONVENTIONAL: u32 = 7; // EfiConventionalMemory: free after ExitBootServices
const IDENTITY_END: u64 = crate::mmu::IDENTITY_END; // RAM the kernel's identity map covers (x86: 4 GiB, aarch64: 1 TiB)
const MIN_RANGE: u64 = 2 * 1024 * 1024;

struct Pool { heaps: [Heap; RANGES], count: usize }
static mut POOL: Pool = Pool { heaps: [const { Heap::empty() }; RANGES], count: 0 };
static LOCK: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);

fn locked<T>(f: impl FnOnce(&mut Pool) -> T) -> T {
    crate::interrupts::without(|| {
        while LOCK.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() { core::hint::spin_loop(); }
        let result = f(unsafe { &mut *core::ptr::addr_of_mut!(POOL) });
        LOCK.store(false, Ordering::Release);
        result
    })
}

/// Takes the largest free conventional ranges the identity map covers. The bootloader allocated everything it hands over (arena,
/// boot images, kernel, memory map) as loader data, so conventional memory is unused.
pub unsafe fn init(map: &[StatPhys]) {
    let mut ranges = [(0u64, 0u64); RANGES];
    for entry in map.iter().filter(|e| e.kind == CONVENTIONAL) {
        let start = entry.start.max(0x10_0000).next_multiple_of(4096);
        let end = (entry.start + entry.pages * 4096).min(IDENTITY_END);
        if end <= start || end - start < MIN_RANGE { continue; }
        // Keep the RANGES largest.
        let smallest = (0..RANGES).min_by_key(|&i| ranges[i].1 - ranges[i].0).unwrap();
        if end - start > ranges[smallest].1 - ranges[smallest].0 { ranges[smallest] = (start, end); }
    }
    locked(|pool| {
        for &(start, end) in ranges.iter().filter(|r| r.1 > r.0) {
            pool.heaps[pool.count].init(start as *mut u8, (end - start) as usize);
            pool.count += 1;
        }
    });
    READY.store(true, Ordering::Release);
}

pub fn ready() -> bool { READY.load(Ordering::Acquire) }

/// Zeroed frames for `layout`, or None when no range has room.
pub fn allocate(layout: Layout) -> Option<NonNull<u8>> {
    let pointer = locked(|pool| pool.heaps[..pool.count].iter_mut().find_map(|heap| heap.allocate_first_fit(layout).ok()))?;
    unsafe { core::ptr::write_bytes(pointer.as_ptr(), 0, layout.size()); }
    Some(pointer)
}

/// # Safety
/// `pointer` and `layout` must come from `allocate`.
pub unsafe fn free(pointer: NonNull<u8>, layout: Layout) {
    let at = pointer.as_ptr() as usize;
    locked(|pool| {
        let heap = pool.heaps[..pool.count].iter_mut().find(|h| (h.bottom() as usize..h.top() as usize).contains(&at)).expect("frames freed outside the pool");
        heap.deallocate(pointer, layout);
    });
}

/// (total, free) bytes of the pool.
pub fn stats() -> (usize, usize) {
    locked(|pool| pool.heaps[..pool.count].iter().fold((0, 0), |(t, f), h| (t + h.size(), f + h.free())))
}
