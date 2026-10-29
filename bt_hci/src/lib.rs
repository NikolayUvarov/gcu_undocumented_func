#![no_std]
//! bt_hci (700-BLT-0001): a Bluetooth controller's HCI over USB. `hci` builds commands and reads events, `rtl` makes
//! a Realtek controller's patch from its files. Both are host-tested (tests/bt_hci_host.rs).

pub mod hci;
pub mod rtl;
