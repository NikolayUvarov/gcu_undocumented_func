use alloc::alloc::{alloc_zeroed, dealloc};
use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;

// Owned kernel RAM. Allocations also serve process-memory syscalls; the global
// allocator disables local IRQs while holding its lock. Task teardown happens
// only after its CPU has switched away from its address space and saved context.
pub struct Region {
    ptr: NonNull<u8>,
    layout: Layout,
    frames: bool, // from the frame pool (task memory), else the kernel arena
}

impl Region {
    pub fn new(size: usize, alignment: usize) -> Result<Self, &'static str> {
        let layout = Layout::from_size_align(size.max(1), alignment)
            .map_err(|_| "INVALID ALLOCATION SIZE")?;
        let ptr = NonNull::new(unsafe { alloc_zeroed(layout) }).ok_or("OUT OF MEMORY")?;
        Ok(Self { ptr, layout, frames: false })
    }

    // Task memory: from the frame pool once it exists (issue 150), else from the arena.
    pub fn task(size: usize, alignment: usize) -> Result<Self, &'static str> {
        if !crate::frames::ready() { return Self::new(size, alignment); }
        let layout = Layout::from_size_align(size.max(1), alignment).map_err(|_| "INVALID ALLOCATION SIZE")?;
        let ptr = crate::frames::allocate(layout).ok_or("OUT OF MEMORY")?;
        Ok(Self { ptr, layout, frames: true })
    }

    pub fn ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }
    pub fn len(&self) -> usize {
        self.layout.size()
    }
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr(), self.len()) }
    }
}

impl Drop for Region {
    fn drop(&mut self) {
        unsafe {
            if self.frames { crate::frames::free(self.ptr, self.layout) } else { dealloc(self.ptr(), self.layout) }
        }
    }
}

/// Each task's kernel structures (the task, its capability table, its page tables' list): from the frame pool and charged
/// to quotas (171-KRN-0032); from the arena before the pool is ready, and on the host.
#[derive(Clone, Copy, Default)]
pub struct Frames;

unsafe impl Allocator for Frames {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let layout = Layout::from_size_align(layout.size().max(1), layout.align()).map_err(|_| AllocError)?;
        let pointer = if crate::frames::ready() { crate::frames::allocate(layout) } else { NonNull::new(unsafe { alloc_zeroed(layout) }) };
        pointer.map(|p| NonNull::slice_from_raw_parts(p, layout.size())).ok_or(AllocError)
    }
    unsafe fn deallocate(&self, pointer: NonNull<u8>, layout: Layout) {
        let layout = Layout::from_size_align_unchecked(layout.size().max(1), layout.align());
        if crate::frames::owns(pointer.as_ptr()) { crate::frames::free(pointer, layout) } else { dealloc(pointer.as_ptr(), layout) }
    }
}
