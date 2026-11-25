# 700-BLT-0005 — Connections carry data: the host stack `bt_host`, ACL both ways, links made and taken, L2CAP

**Type:** driver (`bt_hci`) and library (`bt_host`) · **Owner:** `BLT` · **Priority:** P1 (the maintainer's word of 2026-11-26: Bluetooth closed while the Ryzen is at hand; testing first, hardening later) · **Status:** in progress (2026-11-26) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-7.1, MC-11.4, MC-12.1, MC-12.3

## Problem

`bt_hci` brings a controller up and listens (700-BLT-0001 to 0004), and nothing more:
- no ACL data: the bulk pair is unused;
- no connections, in either direction, on either transport;
- no L2CAP, so no profile can run.

## Plan

1. **`bt_host`, the stack as a library.** It is `no_std`, with no allocation and no `libmind`. Events, ACL packets and the time go in. Commands, ACL packets, log lines and upcalls come out. It has no I/O of its own, so it is tested on the host. `bt_hci` links it into its own process (the testing stage).
2. **The controller (`ctl.rs`).**
   - Commands wait for the controller's credit, `Num_HCI_Command_Packets`. A command that is never answered gives its credit back after 3 s.
   - ACL packets wait for the controller's buffers. `Read Buffer Size` and `LE Read Buffer Size` give the sizes; LE shares BR/EDR's when it has none. `Number Of Completed Packets` returns them, and so does a link's end.
3. **ACL data (`acl.rs`).**
   - The bulk IN endpoint's 64-byte packets are joined by the ACL headers' lengths. A header too long is skipped to its transfer's end.
   - L2CAP PDUs are reassembled from a link's fragments by the packet boundary flag.
   - Outgoing PDUs are cut to the controller's data length.
4. **Links (`stack.rs`).**
   - BR/EDR: `Create Connection` to a device listed, or a `Connection Request` from one accepted (we stay peripheral). Anything else is refused.
   - LE: `LE Create Connection` to an LE device listed (700-BLT-0009 does the rest).
   - A page that fails is tried again after 15, 30 and then 60 s. A link that drops is paged again after 5 s.
5. **L2CAP (`l2cap.rs`).**
   - Basic mode on BR/EDR's signalling channel: connection, configuration (MTU; enhanced retransmission refused, basic named), disconnection, information, echo, command reject. Both as initiator and as responder.
   - A responder's channel may be held "pending" until the link is secure.
   - LE's signalling channel: connection parameter update, checked and applied.
6. **The listening rounds.** These are the 700-BLT-0002/0003 rounds, now run by the stack. Only with `data/bt-listen.txt`, and never while a link is up or a page runs.
7. **The device list.** `data/bluetooth/devices.txt` is the testing stage's: `AA:BB:CC:DD:EE:FF audio|hid|le-hid [random]`. Only a device listed is paged, accepted or paired: the scans hear the neighbours' devices too. Without the file the radio stays as quiet as before (no page scan).

**What is not done here (the hardening stage, later; not to be presented as existing, MC-12.3).**
- **A `bt_host` service.** The plan in 700 has it apart from the transport, with its own capabilities (MC-3.x).
- **A policy for who may connect** and an audit of it (MC-10.x). Here the list file decides.
- **The parsers' isolation from the keys** (MC-11.4). Both are in one process.
- **Host flow control** (controller to host). It is not used: the controller sends as it likes, and the driver polls every 2 ms while a link is up.

## Acceptance criteria

- [x] Host tests (`bt_host/tests/links.rs`):
  - ACL joined across USB packets;
  - PDUs reassembled;
  - the credits and buffers;
  - both directions of a channel's setup;
  - pending admission;
  - configuration options refused or unknown;
  - information, echo, reject;
  - LE parameter updates;
  - the controller's setup;
  - the device list;
  - a page that fails and is tried again;
  - strangers refused.
- [x] `bt_hci` builds for x86_64 and aarch64 with the stack linked.
- [ ] On the DEXP: the stack's setup is accepted (`BR/EDR BUFFERS: …`, `LE BUFFERS: …`, `FEATURES …`), and no command fails. A page to an address that does not exist fails and is tried again.
- [ ] The `usb`, `keys` and `link` QEMU suites pass with this `bt_hci` (no controller: it waits and says none).
- [ ] On the Ryzen, with a device of the maintainer's: a link made and an L2CAP channel opened (the bench, see 700-BLT-0007 and 0008).

## Related

[700](700-bluetooth.md), [700-BLT-0001](700-BLT-0001-bt-hci-the-rtl8821c-up.md), [700-BLT-0006](700-BLT-0006-classic-pairing-and-link-keys.md) (pairing), [700-BLT-0007](700-BLT-0007-a2dp-source.md) (A2DP), [700-BLT-0008](700-BLT-0008-classic-hid-host.md) (HID), [700-BLT-0009](700-BLT-0009-le-pairing-and-hid-over-gatt.md) (LE), [700-DRV-0043](700-DRV-0043-usb-host-hands-out-bluetooth-controllers.md) (the bulk IN endpoint polled).
