//! Process page memory and shared buffers. This is not a small-object allocator.
use crate::abi::*;
use crate::sys::{call, check, syscall, Error, Result};

/// Private zeroed block of pages; freed in Drop (and on process exit).
pub struct Pages { address: usize, length: usize }

impl Pages {
    pub fn new(bytes: usize) -> Option<Self> {
        match call(SYSCALL_ALLOC, bytes, 0) { 0 => None, address => Some(Self { address, length: bytes }) }
    }
    pub fn address(&self) -> usize { self.address }
    pub fn len(&self) -> usize { self.length }
    pub fn is_empty(&self) -> bool { self.length == 0 }
    pub fn as_slice(&self) -> &[u8] { unsafe { core::slice::from_raw_parts(self.address as *const u8, self.length) } }
    pub fn as_mut_slice(&mut self) -> &mut [u8] { unsafe { core::slice::from_raw_parts_mut(self.address as *mut u8, self.length) } }
    /// Capability for this block, to pass to another process over IPC.
    pub fn share(&self) -> Result<usize> { check(call(SYSCALL_MEM_SHARE, self.address, 0)) }
}

impl Drop for Pages { fn drop(&mut self) { call(SYSCALL_FREE, self.address, 0); } }

/// Mapping of another process's memory via a capability; unmapped in Drop.
pub struct Mapping { address: usize, length: usize }

impl Mapping {
    pub fn new(cap_slot: usize) -> Result<Self> {
        let raw = syscall(SYSCALL_MEM_MAP, cap_slot, 0, [0; 4]);
        check(raw.result).map(|address| Self { address, length: raw.arg2 })
    }
    pub fn address(&self) -> usize { self.address }
    pub fn len(&self) -> usize { self.length }
    pub fn is_empty(&self) -> bool { self.length == 0 }
    pub fn as_ptr<T>(&self) -> *mut T { self.address as *mut T }
    pub fn as_slice(&self) -> &[u8] { unsafe { core::slice::from_raw_parts(self.address as *const u8, self.length) } }
    pub fn as_mut_slice(&mut self) -> &mut [u8] { unsafe { core::slice::from_raw_parts_mut(self.address as *mut u8, self.length) } }
}

impl Drop for Mapping { fn drop(&mut self) { call(SYSCALL_FREE, self.address, 0); } }

/// Physical address of a DMA region (only for DMA capabilities granted to a driver by the kernel).
pub fn dma_physical(cap_slot: usize) -> Result<usize> {
    let physical = check(call(SYSCALL_MEM_PHYS, cap_slot, 0))?;
    if physical == 0 { Err(Error::Invalid) } else { Ok(physical) }
}
