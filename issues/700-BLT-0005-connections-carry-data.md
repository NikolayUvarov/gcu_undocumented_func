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

## Progress (2026-11-25, UTC)

- **QEMU.** The `usb`, `keys` and `link` suites pass on x86 (4 CPUs, LEG9, `tests/qemu_smoke.py --suites usb,keys,link`) with this `bt_hci` at e05c89e10. QEMU has no controller, and `bt_hci` sleeps in its claim loop.
- **The DEXP, a release of this branch** (202611252251, tree bcb794384 on main 219b9da78, no device listed), through the Effector server's `update_agent`:
  - The agent took the operation at 22:51:53 and downloaded the bundle.
  - Its last heartbeat was at 22:57:58, with `shutting_down` (the apply's restart).
  - It had not answered the server again by 23:14, so the run was stopped there and reported to the owner session (manageability first).
  - Whether the trial boot or the fallback to slot A is what does not come back is unknown from here. No log of either boot has been read.
- **Delivery to the DEXP.** An empty device list was delivered to its `data/inbox/bt-devices.txt` (22:49). The server refuses `data/bluetooth/`.

## Review of the start path against a real controller (2026-11-25, after the DEXP did not come back)

**Main's `bt_hci` at 219b9da78 with no `data/bt-listen.txt`:** the bring-up, then `SET_EVENT_MASK` and `WRITE_INQUIRY_MODE`, then `idle()`. It made no further `usb_host` call, and its bulk IN endpoint was never polled.

**The build released to the DEXP (bcb794384), nothing listed:** the bring-up the same, then always the stack.
- **The stack's setup,** each command once, never retried:
  - `READ_LOCAL_FEATURES`, `READ_BUFFER_SIZE`;
  - `SET_EVENT_MASK`, with the SSP bits 46-53 and 58;
  - `WRITE_SIMPLE_PAIRING_MODE` 1, `WRITE_INQUIRY_MODE` 2;
  - `WRITE_CLASS_OF_DEVICE`, `WRITE_LOCAL_NAME` (3 + 248 bytes, under the patch fragments' 3 + 253);
  - `WRITE_DEFAULT_LINK_POLICY` 5, `WRITE_PAGE_TIMEOUT`;
  - `LE_SET_EVENT_MASK`, `WRITE_LE_HOST_SUPPORTED` (1, 0), `LE_READ_BUFFER_SIZE`.
- **Then a loop that never ended,** every 20 ms. Each turn made two `usb_host` calls: `reports` on the interrupt endpoint, and `reports` on the bulk IN endpoint. The first bulk call armed that endpoint, its first use on any machine. That is about 100 calls a second, as much as one more HID device.
- **What it did not do:**
  - every wait sleeps 20 ms, or 10 ms in the bring-up;
  - a command not answered is given up after 3 s, an error is logged, nothing is retried;
  - event floods are bounded by usb_host's eight packets a call;
  - `sleep` returns early only with keyboard input queued, which a background service never has.
- **The polled endpoints.** The DEXP's last log polls five endpoints at start (`usb_host`'s limit is eight); the bulk IN endpoint made six.
- **The stack and the image.** `PT_GNU_STACK` grew from the default 64 KiB to 256 KiB (now 512 KiB). `bt_hci.elf` grew from 132,896 bytes (main's, built alone from 219b9da78) to about 268,600 in the released build. With LE and the sink it is 365,528 now.

**Could it have hung the DEXP?** I found no mechanism. `bt_hci` is a ring-3 client that sleeps between calls, and the kernel's trial restart (120 s) is beyond its reach. That the machine did not come back even through the fall-back to slot A points below it. This is a judgment, not shown.

**What else came with the release.** The DEXP ran de3eac32 (2026-11-21, ABI 34); the release is 511 commits of main on top (ABI 37). Among them:
- `usb_host` +600 lines: deferred bulk transfers (000-DRV-0101) on the boot stick's xHCI, bulk streams, spill;
- the kernel's scheduler, interrupts and trial;
- `init`, the updater and the bootloader;
- the new `display` service.

**Fixed (bt_host::driver).**
- With no device listed and no listening asked, `bt_hci` sends main's last two commands and idles with no `usb_host` call: main's footprint, command for command.
- On the active path, the bulk IN endpoint is polled only while a link is up.
- A controller gone (or 50 failed polls in a row) stops the loop.
- The whole bring-up and loop moved into `bt_host::driver` behind a port, and run on the host in `bt_host/tests/driver.rs`. A scripted RTL8821C-like and RTL8852B-like controller answers normally, answers errors, answers nothing, or floods events, with the time advanced by the harness.
- The sequences recorded there:
  - **quiet:** `RESET`, `READ_LOCAL_VERSION`, `READ_ROM_VERSION`, n × `DOWNLOAD`, `RESET`, `READ_LOCAL_VERSION`, `READ_BD_ADDR`, `SET_EVENT_MASK`, `WRITE_INQUIRY_MODE`, then nothing;
  - **active:** `1003 1005 0C01 0C56 0C45 0C24 0C13 080F 0C18 0C1A 2001 0C6D 2002`, then `0405` for a device listed. Events are polled at 50 a second, the bulk IN endpoint never without a link.

## Related

[700](700-bluetooth.md), [700-BLT-0001](700-BLT-0001-bt-hci-the-rtl8821c-up.md), [700-BLT-0006](700-BLT-0006-classic-pairing-and-link-keys.md) (pairing), [700-BLT-0007](700-BLT-0007-a2dp-source.md) (A2DP), [700-BLT-0008](700-BLT-0008-classic-hid-host.md) (HID), [700-BLT-0009](700-BLT-0009-le-pairing-and-hid-over-gatt.md) (LE), [700-DRV-0043](700-DRV-0043-usb-host-hands-out-bluetooth-controllers.md) (the bulk IN endpoint polled).
