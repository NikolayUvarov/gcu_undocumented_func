// Physical frames for task memory (issue 150): the free RAM of the firmware map outside the kernel arena. Heap blocks,
// memory objects, screens, images and stacks come from here, and each task's kernel structures (`memory::Frames`).
use crate::abi::StatPhys;
use core::alloc::Layout;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};
use alloc::vec::Vec;
use linked_list_allocator::Heap;

const CONVENTIONAL: u32 = 7; // EfiConventionalMemory: free after ExitBootServices
const IDENTITY_END: u64 = crate::mmu::IDENTITY_END; // what the identity map covers whole (x86: 4 GiB, aarch64: 1 TiB); x86 RAM above: mmu::high_ram
const MIN_RANGE: u64 = 2 * 1024 * 1024;

// One heap for each free range of the firmware map, as many as it lists (issue 171: no fixed count).
struct Pool { heaps: Vec<Heap> }
static mut POOL: Pool = Pool { heaps: Vec::new() };
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

/// Takes every free conventional range the kernel maps, highest first. The bootloader allocated everything it hands
/// over (arena, boot images, kernel, memory map) as loader data, so conventional memory is unused.
pub unsafe fn init(map: &[StatPhys]) {
    let mut ranges: Vec<(u64, u64)> = Vec::new();
    let mut take = |start: u64, end: u64| if end > start && end - start >= MIN_RANGE { ranges.push((start, end)); };
    for entry in map.iter().filter(|e| e.kind == CONVENTIONAL) {
        let (start, end) = (entry.start.max(0x10_0000).next_multiple_of(4096), entry.start + entry.pages * 4096);
        take(start, end.min(IDENTITY_END));
        // x86-64: RAM above 4 GiB, as far as the kernel maps it (issue 171).
        if let Some((high, top)) = crate::mmu::high_ram(start, end) { take(high, top); }
    }
    // Highest first: task memory comes from above 4 GiB while there is some; nothing a device reaches is task memory.
    ranges.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    locked(|pool| {
        pool.heaps.reserve_exact(ranges.len());
        for &(start, end) in &ranges {
            let mut heap = Heap::empty();
            heap.init(start as *mut u8, (end - start) as usize);
            pool.heaps.push(heap);
        }
    });
    READY.store(true, Ordering::Release);
}

pub fn ready() -> bool { READY.load(Ordering::Acquire) }

/// Zeroed frames for `layout`, or None when no range has room.
pub fn allocate(layout: Layout) -> Option<NonNull<u8>> {
    let pointer = locked(|pool| pool.heaps.iter_mut().find_map(|heap| heap.allocate_first_fit(layout).ok()))?;
    unsafe { core::ptr::write_bytes(pointer.as_ptr(), 0, layout.size()); }
    Some(pointer)
}

/// # Safety
/// `pointer` and `layout` must come from `allocate`.
pub unsafe fn free(pointer: NonNull<u8>, layout: Layout) {
    let at = pointer.as_ptr() as usize;
    locked(|pool| {
        let heap = pool.heaps.iter_mut().find(|h| (h.bottom() as usize..h.top() as usize).contains(&at)).expect("frames freed outside the pool");
        heap.deallocate(pointer, layout);
    });
}

/// Whether `pointer` lies in the pool (and so goes back to it).
pub fn owns(pointer: *const u8) -> bool {
    let at = pointer as usize;
    ready() && locked(|pool| pool.heaps.iter().any(|h| (h.bottom() as usize..h.top() as usize).contains(&at)))
}

/// (total, free) bytes of the pool.
pub fn stats() -> (usize, usize) {
    locked(|pool| pool.heaps.iter().fold((0, 0), |(t, f), h| (t + h.size(), f + h.free())))
}
