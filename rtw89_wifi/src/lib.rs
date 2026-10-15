#![no_std]

//! RTL8852BE firmware parsing and bounded initial board access; no radio or host DMA startup.

pub mod board;
pub mod btc;

pub mod btc_init;
pub mod dack;
mod dack_tables;
pub mod efuse;
pub mod download;
pub mod firmware;
pub mod fwcmd;
pub mod h2c;
pub mod io;
pub mod mac;
pub mod pcie;
pub mod power;
pub mod phy;
mod phy_tables;
pub mod registers;
pub mod runtime_mac;
mod runtime_mac_regs;
pub mod rx;
pub mod rxdesc;
pub mod c2h;
pub mod post_mac;
mod post_mac_regs;

pub mod rxdck;
