#![no_std]

//! RTL8852BE firmware, bounded device initialization and retained DMA helpers.

pub mod board;
pub mod calibration;
pub mod channel_policy;
#[allow(dead_code)]
pub(crate) mod channel_power;
mod power_tables;
pub mod btc;
pub mod btc_codec;
pub mod btc_control;
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
pub mod phycap;
mod runtime_mac_regs;
pub mod rx;
pub mod rxdesc;
pub mod c2h;
pub mod btc_reports;
pub mod post_mac;
mod post_mac_regs;

#[allow(dead_code)]
pub(crate) mod report_tail;

pub mod rxdck;

pub mod btc_session;

pub mod btc_context;
