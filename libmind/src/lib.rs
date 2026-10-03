#![no_std]
//! libmind is the MIND CORE program SDK: system calls, IPC, memory, devices, graphics and service clients.

#[path = "../../common/abi.rs"]
pub mod abi;
#[path = "../../common/font.rs"]
pub mod font;

pub mod audio;
pub mod block;
pub mod control;
pub mod dev;
pub mod fs;
pub mod gfx;
pub mod input;
pub mod ipc;
pub mod mem;
pub mod platform;
pub mod process;
pub mod rtc;
pub mod sys;
pub mod time;
pub mod tts;
pub mod util;
#[doc(hidden)]
pub mod rt;

pub use abi::BootInfo;
pub use sys::{Error, Result};

/// Declares the `_start` entry point, initializes the mailbox and exits the process after `main`.
#[macro_export]
macro_rules! entry {
    ($main:path) => {
        #[no_mangle]
        #[link_section = ".text._start"]
        pub extern "sysv64" fn _start(info: &'static $crate::abi::BootInfo, mailbox: *mut $crate::abi::SyscallMailbox) -> ! {
            unsafe { $crate::sys::init(mailbox) };
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
