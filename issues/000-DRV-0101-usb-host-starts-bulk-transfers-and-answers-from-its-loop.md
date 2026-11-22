# 000-DRV-0101 — `usb_host` starts bulk transfers and answers from its loop instead of waiting on them

**Type:** driver (`usb_host`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-22: the issue only, by the owner session's decision) · **Blocked by:** — · **Roadmap:** A · **Constitution:** MC-6.1, MC-11.4, MC-12.1

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

- [ ] Host or QEMU cases: a bulk transfer answered from the loop, one that halts, one that times out, a client gone before its answer.
- [ ] QEMU's `usb`, `ehci`, `cameras`, `link`, `usbaudio` and the USB storage suites pass.
- [ ] On the Ryzen: a duplex with a log read and a stick write in its middle, every stream's `TIMES DRY` 0 while they run, and the stick's and the cable's throughput no lower (MC-12.1).

## Related

[000-DRV-0094](000-DRV-0094-an-isochronous-in-ring-does-not-run-dry-while-its-client-is-slow.md), [000-LNK-0050](000-LNK-0050-usb-serial-writes-in-pieces-so-usb-host-serves-its-streams.md), [158-DRV-0082](158-DRV-0082-a-stream-for-each-camera.md), [000-DRV-0073](000-DRV-0073-usb-audio-class-speakerphone.md).
