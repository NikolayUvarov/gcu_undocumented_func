#![no_std]

//! The RTL8821CE's Wi-Fi (550-DRV-0042), ported from Linux's rtw88 under its BSD-3-Clause option.

pub mod coex;
pub mod cond;
pub mod dma;
pub mod efuse;
pub mod fw;
pub mod h2c;
pub mod io;
pub mod load;
pub mod mac;
pub mod phy;
pub mod power;
pub mod radio;
pub mod tables;
pub mod txrx;
