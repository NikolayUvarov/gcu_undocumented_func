use alloc::alloc::{alloc_zeroed, dealloc};
use core::alloc::Layout;
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

/// A kernel structure in task memory, the frame pool, so the arena does not bound how many there are (issue 171).
pub struct Boxed<T> {
    region: Region,
    _type: core::marker::PhantomData<T>,
}

impl<T> Boxed<T> {
    /// Room for a `T`, taken before the value exists (a spawn charges it first).
    pub fn room() -> Result<Region, &'static str> { Region::task(core::mem::size_of::<T>(), core::mem::align_of::<T>()) }
    pub fn place(region: Region, value: T) -> Self {
        assert!(region.len() >= core::mem::size_of::<T>() && region.ptr() as usize % core::mem::align_of::<T>() == 0);
        unsafe { (region.ptr() as *mut T).write(value); }
        Self { region, _type: core::marker::PhantomData }
    }
    pub fn bytes(&self) -> usize { self.region.len() }
}

impl<T> core::ops::Deref for Boxed<T> {
    type Target = T;
    fn deref(&self) -> &T { unsafe { &*(self.region.ptr() as *const T) } }
}

impl<T> core::ops::DerefMut for Boxed<T> {
    fn deref_mut(&mut self) -> &mut T { unsafe { &mut *(self.region.ptr() as *mut T) } }
}

impl<T> Drop for Boxed<T> {
    fn drop(&mut self) { unsafe { core::ptr::drop_in_place(self.region.ptr() as *mut T); } }
}
