#![no_std]

//! RTL8852BE firmware, bounded device initialization and retained DMA helpers.

pub mod board;
pub mod efuse;
pub mod download;
pub mod firmware;
pub mod fwcmd;
pub mod io;
pub mod mac;
pub mod pcie;
pub mod power;
pub mod registers;
pub mod runtime_mac;
mod runtime_mac_regs;
pub mod rx;
pub mod rxdesc;
pub mod c2h;
pub mod post_mac;
mod post_mac_regs;
