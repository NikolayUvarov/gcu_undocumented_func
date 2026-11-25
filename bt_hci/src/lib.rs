#![no_std]
//! bt_hci (700-BLT-0001): a Bluetooth controller's HCI over USB. `hci` (now bt_host's, shared with the host stack)
//! builds commands and reads events, `rtl` makes a Realtek controller's patch from its files. Both are host-tested
//! (tests/bt_hci_host.rs); the stack is bt_host's (bt_host/tests/).

pub use bt_host::hci;
pub mod rtl;
