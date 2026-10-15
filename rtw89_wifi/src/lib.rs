#![no_std]

//! RTL8852BE firmware parsing and bounded initial board access; no radio or host DMA startup.

pub mod board;
pub mod efuse;
pub mod download;
pub mod firmware;
pub mod fwcmd;
pub mod io;
pub mod mac;
pub mod pcie;
pub mod power;
pub mod phy;
mod phy_tables;
pub mod registers;
