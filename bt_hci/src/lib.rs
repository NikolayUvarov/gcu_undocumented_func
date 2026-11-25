#![no_std]
//! bt_hci (700-BLT-0001): a Bluetooth controller's HCI over USB. `hci` builds commands and reads events, `rtl` makes a
//! Realtek controller's patch from its files; both are now bt_host's (and host-tested by tests/bt_hci_host.rs), as is
//! the driver's logic (bt_host::driver, bt_host/tests/driver.rs). This crate is its port to usb_host.

pub use bt_host::{hci, rtl};
