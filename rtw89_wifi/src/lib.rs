#![no_std]

//! RTL8852BE firmware parsing and bounded initial board access; no radio or host DMA startup.

pub mod board;
pub mod dack;
mod dack_tables;
pub mod efuse;
pub mod firmware;
pub mod io;
pub mod power;
pub mod phy;
mod phy_tables;
pub mod registers;
