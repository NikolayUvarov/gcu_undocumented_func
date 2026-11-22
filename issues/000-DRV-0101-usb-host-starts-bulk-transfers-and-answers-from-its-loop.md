# 000-DRV-0101 — `usb_host` starts bulk transfers and answers from its loop instead of waiting on them

**Type:** driver (`usb_host`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-22: written on `claude/000-DRV-0101-usb-host-answers-bulk-from-its-loop` at the owner session's word, and through the host tests and QEMU's suites; no hardware run before the owner session says) · **Blocked by:** — · **Roadmap:** A · **Constitution:** MC-6.1, MC-11.4, MC-12.1

## Problem

`usb_host` serves one request at a time. A client's bulk transfer is waited on in `completion_for` until it ends: up to 30 s.
- **What it holds meanwhile:**
  - every other client's request: a take or a put of an isochronous stream, a camera's bulk stream, a keyboard's report;
  - the isochronous OUT rings, which only the client's puts fill;
  - before 000-DRV-0094's step 2, also the isochronous and bulk IN rings, which only the clients' takes, or the main loop, put back.
- **What today's runs on the Ryzen showed** (2026-11-22, release 202611220957, raw lines in `S/handoff/cam/0094-runs-C-D-and-cameras.txt`):
  - one log read over the cable (the CH340's bulk OUT) made `usb_host` wait 30–45 ms a write;
  - every one of 64 dry rings of the STM-Speaker board in run C fell in such a wait (`WAITING ON SLOT 3 ENDPOINT 4`);
  - the board counted 9216 capture frames overflowed and 2484 playback frames short in those six seconds;
  - in run D, with step 2, the IN rings stayed full but playback ran dry 12 times, every one waiting on the CH340.
- **The same path** carries the boot stick's reads and writes (`usb_storage`: the system log and the store), the cameras' controls and the converter's reads. A camera's whole YUY2 frames broke by its own error bit before step 2 refilled the bulk rings during waits, and hardly after (158-DRV-0082). That was one run each, and the waits behind them are a guess.
- **The interim:** 000-LNK-0050 cuts the converter's writes into pieces of 256 bytes. That shortens each wait, but every other bulk client still holds `usb_host` for as long as its transfer runs.

## Plan

1. **A bulk transfer is started, not waited on.** Its TRBs go on the ring and the doorbell rings, and the request's answer is deferred (`wire::Call::defer`, as `video_gw` defers its reads) with the transfer's slot, endpoint and the client's buffer.
2. **The loop answers it** when its completion comes (`pump`), with the bytes moved or the error, and resets a halted endpoint as now.
3. **A transfer with no completion in its time** (30 s, as now) is stopped and answered as timed out, its endpoint's ring moved past it.
4. **What stays synchronous, said:** control transfers of set-up, and commands. Each is said with why, or made the same way.
5. **Clients see the same interface** (`usb.wit` unchanged): only when their answer comes changes.

## Acceptance criteria

- [x] Host or QEMU cases: a bulk transfer answered from the loop, one that halts, one that times out, a client gone before its answer. (2026-11-22: answered from the loop in QEMU's `usbload`; the rules for a halt, a timeout, a device gone and a client's new instance in `tests/usb_pending_host.rs`. Their actions, the endpoint's reset or stop, are the synchronous path's as before.)
- [x] QEMU's `usb`, `ehci`, `cameras`, `link`, `usbaudio` and the USB storage suites pass. (2026-11-22, LEG9, on 05bc8e92c: those, `iso` with its USB disk's boot, `vfs`, `block` with its USB part, and the new `usbload`.)
- [ ] On the Ryzen: a duplex with a log read and a stick write in its middle, every stream's `TIMES DRY` 0 while they run, and the stick's and the cable's throughput no lower (MC-12.1).

## Progress (2026-11-22)

**What is written** (`usb_host/src/main.rs`, `usb_host/src/pending.rs`, `usb_host/src/xhci.rs`):
- **Starting a transfer:** a client's bulk transfer on xHCI is put on its ring (`Xhci::start`), and `usb_host` looks for its completion for up to 2 ms (`BULK_SPIN_NS`), taking the events and putting the streams' TDs back meanwhile (`Xhci::finished`).
  - One that ends within that is answered at once, as before. The stick's reads mostly do.
  - A longer one has its answer deferred (`wire::Call::defer`) into a table of 8 (`Pending`), and `usb_host` goes back to its loop.
- **The loop:**
  - it waits 1 ms at most while a transfer is deferred;
  - each turn it answers those that ended (`bulks`), copying an IN transfer's bytes into the client's buffer, and resets the endpoint after an error as before;
  - it answers not-found one whose device or controller went;
  - it stops one with no completion for 30 s and answers it as timed out, its TRB left behind as before;
  - a new instance of a client that attaches has its earlier instance's transfers stopped and answered first.
  - The first transfer answered from the loop is said: `A BULK TRANSFER ANSWERED FROM THE LOOP: SLOT s ENDPOINT e, n BYTES, m MS`.
- **The bytes' place** (`pending::place`): a pool page of its own for up to a page (the serial writes), else DATA, which one transfer a controller holds at a time. A long one waits for DATA while another holds it, so a stick's read and a converter's write go on together.
- **No room to defer** (8 deferred already): the transfer is waited on, as before.
- **EHCI's bulk transfers** stay waited on: the Ryzen's devices are all on its xHCIs.

**What still blocks the loop, and why it stays for now:**
- **Control transfers** (`control_request`): waited on, with the streams' TDs put back meanwhile (0094's step 2). They end within a few milliseconds: a camera's probe and commit took 6 ms on the Ryzen.
- **Commands** (`Xhci::command`): Configure Endpoint, Stop and Reset Endpoint, Set Dequeue. They come at a device's set-up, an interface's setting and an error's recovery, and take a few milliseconds: 8 ms at capture's close on the Ryzen.
- **A port's reset and its recovery** during a scan, 20 to 50 ms: only when a device comes.
- Each of these could be answered from the loop the same way, `select` with its control transfer and its command among them. That is a larger change than the bulk path, and none of them recurs while streams run.

**000-DRV-0081 is not covered by this.** Its 38 ms at capture's close (0094's step 1 run: `WAITING ON A COMMAND FOR 8.0 MS`, the client's calls 32 ms before and 30 ms after) was two things:
- the command, 8 ms;
- `audio_gw` not taking or putting while it closed capture, about 30 ms.

0094's step 2 already keeps the IN rings full through both: in run D the feedback ring did not run dry at the close. The playback ring's dry at the close (run D: 12.5 ms, `NOT WAITING`) is the client stopping its puts while it closes. 0081 still needs its own change: SET_INTERFACE 0 before the endpoints are dropped, and the stream marked closing so that its last transfers are not counted as errors. That order is the speakerphone's 82 to 92 frames lacked at each close.

**Tests:**
- **Host:** `tests/usb_pending_host.rs`, the place of a transfer's bytes and the loop's next step.
- **QEMU:** a new suite `usbload`, in the gate's "services, storage, audio" group. It boots from a USB stick with a `usb_host` built to answer every bulk transfer from its loop (`defer-every-bulk`: QEMU ends transfers at once, so the default build would answer them on the spot). While QEMU's usb-audio plays audiostub's tone for 20 s and the host reads the system log over QEMU's FT232, the shell reads a 4 MiB file off the stick three times. Every read must be whole (SHA-256), the link's read whole, and the tone played to its end.
- Then QEMU's `usb`, `ehci`, `iso`, `cameras`, `link`, `usbaudio`, `vfs` and `block` (whose USB part writes past a file system through the stick), and the full host tests.
- **Results** (2026-11-22, LEG9, 05bc8e92c, run twice, the second after LEG9's restart):
  - the full `host_tests.sh` passed whole (263 groups);
  - QEMU gave 17 PASS and 0 FAIL: `usb`, `ehci`, `iso`, `cameras`, `link`, `usbaudio` with the audiostub check, `vfs`'s eight, `block` (ATA, AHCI and USB) and `usbload`, which passed three times in all;
  - in `usbload` the first transfer answered from the loop was the stick's first command, `SLOT 1 ENDPOINT 02, 31 BYTES, 1 MS`.
- **Criterion 1's rest, host cases:** `pending::answer` decides what a deferred transfer's client is answered: its bytes, a reset after a halt, its device or controller gone, a timeout, or its client's new instance. The loop and the new instance's attach act on that decision. `tests/usb_pending_host.rs` holds a case for each, and for which outweighs which.
  - **Not tested on any machine:** a real halt, a real timeout or a client's death while a transfer is deferred. QEMU's devices do not stall on demand.
- **The tally** (3ad6e4d76): `BULK TRANSFERS FROM THE LOOP IN THE LAST MINUTE: n DEFERRED, a ANSWERED, f NOT FOUND, t TIMED OUT, s STOPPED, w WAITED ON; THE LONGEST m MS`, said once a minute when there were any, for the hardware run.

**On the Ryzen** (release 202611221738, boot 97 from 17:42:50 UTC, the bench tree with 3ad6e4d76; read over the cable by the owner session and CAM):
- The first transfer answered from the loop came at 3.533 s: `SLOT 3 ENDPOINT 81, 65536 BYTES, 3 MS`, a 64 KiB read of the stick.
- The tally, minute by minute, at uptime 60, 120 and 180 s: 157, 413 and 2914 deferred, every one answered, 0 not found, 0 timed out, 0 stopped, 0 waited on, the longest 6, 14 and 17 ms.
  - The third minute holds eight timed log reads over the cable, about 600 KB through the CH340's pieces.
- The stick, the CH340 and both cameras were found. There was no stop-list line (`usb_storage`'s no-answer, `vfs_server`'s drive not answering, the converter gone, an endpoint reset, a controller error, the Effector journal) to uptime 162 s.
- **Not shown:**
  - a ring kept full by it: no speakerphone was on the machine and no camera streamed, so no isochronous or bulk-stream ring existed;
  - the link's throughput against before: the young boot's log gave reads of 76 KB, about 30 KB/s, not the earlier 137.8 KB.
- It holds for that machine and that release (MC-12.1).

## Related

[000-DRV-0094](000-DRV-0094-an-isochronous-in-ring-does-not-run-dry-while-its-client-is-slow.md), [000-LNK-0050](000-LNK-0050-usb-serial-writes-in-pieces-so-usb-host-serves-its-streams.md), [158-DRV-0082](158-DRV-0082-a-stream-for-each-camera.md), [000-DRV-0073](000-DRV-0073-usb-audio-class-speakerphone.md).
