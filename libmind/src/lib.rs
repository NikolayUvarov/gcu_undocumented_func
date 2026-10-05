#![no_std]
//! libmind is the MIND CORE program SDK: system calls, IPC, memory, devices, graphics and service clients.
#[cfg(feature = "alloc")]
extern crate alloc;

#[path = "../../common/abi.rs"]
pub mod abi;
#[path = "../../common/font.rs"]
pub mod font;
#[path = "../../common/font16.rs"]
pub mod font16;

pub mod audio;
pub mod block;
pub mod block_protocol;
pub mod control;
pub mod dev;
pub mod fs;
pub mod gfx;
pub mod heap;
pub mod idl;
pub mod input;
pub mod ipc;
pub mod keys;
pub mod log;
pub mod mask;
pub mod mem;
pub mod network;
pub mod netring;
pub mod output;
pub mod platform;
pub mod process;
mod arch;
pub mod random;
pub mod rtc;
pub mod stat;
pub mod sys;
pub mod time;
pub mod tts;
pub mod tui;
pub mod util;
pub mod window;
pub mod windowed;
#[cfg(feature = "alloc")]
pub mod pattern;
#[cfg(feature = "alloc")]
pub mod voice;
pub mod virtio;
#[doc(hidden)]
pub mod rt;

pub use abi::BootInfo;
pub use sys::{Error, Result};

/// With the `alloc` feature the program gets a heap (`Vec`, `String`, `Box` after `extern crate alloc;`).
#[cfg(feature = "alloc")]
mod global {
    use crate::abi::{SYSCALL_ALLOC, SYSCALL_FREE};
    use crate::heap::{Heap, PageSource, Stats};
    use core::alloc::{GlobalAlloc, Layout};
    use core::cell::UnsafeCell;

    pub struct Kernel;
    impl PageSource for Kernel {
        fn alloc(&mut self, bytes: usize) -> usize { crate::sys::call(SYSCALL_ALLOC, bytes, 0) }
        fn free(&mut self, address: usize, _bytes: usize) { crate::sys::call(SYSCALL_FREE, address, 0); }
    }

    pub struct Global(UnsafeCell<Heap<Kernel>>);
    unsafe impl Sync for Global {} // processes are single-threaded
    unsafe impl GlobalAlloc for Global {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 { (*self.0.get()).alloc(layout) }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) { (*self.0.get()).dealloc(pointer, layout) }
    }

    #[global_allocator]
    static HEAP: Global = Global(UnsafeCell::new(Heap::new(Kernel)));

    /// Arenas, large blocks and pages in use by this program's heap.
    pub fn stats() -> Stats { unsafe { (*HEAP.0.get()).stats() } }
}
#[cfg(feature = "alloc")]
pub use global::stats as heap_stats;

/// Declares the `_start` entry point, initializes the mailbox and exits the process after `main`.
#[macro_export]
macro_rules! entry {
    ($main:path) => {
        #[no_mangle]
        #[link_section = ".text._start"]
        pub extern "sysv64" fn _start(info: &'static $crate::abi::BootInfo, mailbox: *mut $crate::abi::SyscallMailbox) -> ! {
            unsafe { $crate::sys::init(mailbox) };
            $crate::log::prepare();
            $crate::output::prepare();
            let main: fn(&'static $crate::abi::BootInfo) = $main;
            main(info);
            $crate::process::exit()
        }
    };
}

/// Formatted output to the process log (LOGS command, UART for the active program).
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{ let _ = core::fmt::Write::write_fmt(&mut $crate::process::Log, format_args!($($arg)*)); }};
}
#[macro_export]
macro_rules! println {
    () => { $crate::process::log(b"\n") };
    ($($arg:tt)*) => {{ $crate::print!($($arg)*); $crate::process::log(b"\n"); }};
}
