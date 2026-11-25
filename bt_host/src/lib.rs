#![no_std]
//! bt_host: the Bluetooth host stack as a library (700-BLT-0005 and on), for the testing stage. Pure state machines
//! and parsers: events, ACL data and the time in; commands, ACL data, log lines and upcalls out; no I/O, no
//! allocation, so it is host-tested (bt_host/tests/) against constructed HCI traffic. `bt_hci` links it into its own
//! process for now. A `bt_host` service apart from the transport, with its own capabilities, link keys sealed by the
//! keystore, a pairing policy and an audit (issue 700's plan; MC-3.x, MC-10.x, MC-11.4) is the hardening stage: it does
//! not exist yet.
//!
//! - `hci`: commands and events; `ctl`: the controller's command credit and ACL buffers; `acl`: ACL packets and L2CAP
//!   reassembly; `l2cap`: channels and signalling;
//! - `stack`: links, pairing, dispatch; `devices`: the devices listed and their keys; `scan`: the listening rounds;
//! - `sdp`: the SDP server; `avdtp` and `sbc`: the A2DP source and its encoder; `hid`: the classic HID host;
//!   `le`: the LE central.

pub mod acl;
pub mod avdtp;
pub mod buf;
pub mod ctl;
pub mod devices;
pub mod hci;
pub mod hid;
pub mod l2cap;
pub mod le;
pub mod sbc;
pub mod scan;
pub mod sdp;
pub mod smp_crypto;
pub mod stack;
