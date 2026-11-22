# 000-DRV-0094 — `usb_host`: an isochronous IN ring does not run dry while its client is slow

**Type:** driver (`usb_host`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-22: steps 1 and 2 written and run on the Ryzen; steps 3 and 4 to come) · **Blocked by:** — · **Roadmap:** A · **Constitution:** MC-6.1, MC-11.4, MC-12.1, MC-12.2

## Problem

Run A of 000-DRV-0093 (the STM-Speaker board `CAFE:4014` on the Ryzen, 2026-11-22 06:54:47–07:09:50 UTC, release 202611220644, `S/handoff/cam/stm-run-A.txt`) kept capture and playback open for 15 minutes:
- **The counts.** Capture (DCI 3, endpoint 81) and feedback (DCI 5, endpoint 82) each ran dry 6 times, with 0 missed and 0 errors. Playback (DCI 2) never ran dry.
- **Only the first dry ring of each stream has a time.** `usb_host` says each completion code once a stream: DCI 3 at uptime 354.844 s, DCI 5 at 356.173 s, both within a few seconds of a 155 KB log read over the cable. The other five of each have no time.
- **What each one costs.** A dry ring (completion 15, Ring Overrun) means no TD was on the ring for a service interval, so the controller sent no IN token. The next TD then starts `FIRST_AHEAD`, 64 microframes, ahead of the controller's frame. So each dry ring is a gap of at least 8 ms with no packet asked for.
- **Why the ring empties.** An IN stream's TDs go back on the ring only when its client (`audio_gw`) takes their packets. A client that comes late, or a `usb_host` busy elsewhere, leaves the ring empty.
- **What the board saw.** The board's own reading of run A found no harm to its processing: 0 restarts of its sound path, and 0 processing blocks over 8 ms or over its 5.6 ms goal. Its USB queue counters were not timed then, so it cannot say whether it saw the pauses.

A guess, not shown: `usb_host` waits for a synchronous transfer of another device without serving a request meanwhile. The cable's converter (`usb_serial`, a CH340 on the same hub) sent the log read's 155 KB that way. A client's take then waits until that transfer ends.

## Plan

1. **Every dry ring said, with its uptime and how long the ring stood empty** (alone, first).
   - **The line**, said when the ring gets its next TD: `DRY n AT s S: NO TD t MS`. It holds:
     - how long the ring stood empty, from the last TD's end to the next TD's frame;
     - the last TD's end before the dry;
     - the client's last call before the dry, and its next call after;
     - the wait `usb_host` was in when the controller said so (a transfer's slot and endpoint, or a command, and for how long), or `NOT WAITING`.
   - Up to 64 such lines a stream; the rest are counted.
   - **The stream's end line** adds the time with no TD on the ring in all, and the longest.
   - A bulk stream's ring that stood empty is said the same way (158-DRV-0082).
2. **`usb_host` puts an IN stream's finished TDs back on the ring by itself**, as the bulk streams do (158-DRV-0082). It keeps their packets in a bounded spill until the client takes them, so a slow client loses packets only once the spill is full, and the endpoint keeps being asked.
3. **The restart after a dry ring no further ahead than the controller needs.** Is 64 microframes the minimum on this xHCI, or a safe guess? The xHCI's Isochronous Scheduling Threshold (HCSPARAMS2 IST) bounds it; the Ryzen's value is to be read and kept to.

4. **An OUT stream's ring kept full while `usb_host` waits for another transfer** (found by run D): `usb_host` holds packets the client gave ahead of the ring, and puts them on as TDs come free, during a wait too.

## Acceptance criteria

- [x] Step 1 on the Ryzen: a duplex run as run A (`audiostub duplex 900 0`, the board behind the Genesys hub), in which every dry ring is said with its time, its length and the wait it fell in. The STM-Speaker agent holds each pause against its own counters, so the run's start and end go to it.
- [ ] Steps 2 to 4, in QEMU's `usbaudio` suite (its `iso` suite is the hybrid ISO's boot, not isochronous streams).
- [ ] The same 15-minute run with 0 TIMES DRY on both IN endpoints (DCI 3 and 5), with nobody else on the cable. That holds for that machine, that board and that release (MC-12.1).

## Progress (2026-11-22)

- **Step 1:** 3eb13b74c. QEMU's `usbaudio` suite and the full host tests passed on LEG9, though no ring ran dry there, so the lines were first seen on the Ryzen.
- **Its run on the Ryzen, 2026-11-22 08:49:13–09:04:17 UTC** by LEG9's clock:
  - release 202611220741, with `usb_host` and `video_gw` of 3eb13b74c by `replace`, then put back;
  - the board behind the Genesys hub, `audiostub duplex 900 0` by the cable's `run`;
  - nobody on the cable during it: no log read and no command;
  - the raw lines are in `S/handoff/cam/0094-step1-run.txt`.
- **During the 900 s no ring ran dry:**
  - capture (DCI 3) 0 times dry over 86 387 594 bytes;
  - feedback (DCI 5) and playback (DCI 2) had none before the close;
  - 0 missed and 0 errors on all three;
  - the stub read 47 991 frames a second with 0 overflows.
- **Two dry rings came at the close,** both at uptime 4741.675, 8 ms after capture's stream ended:
  - feedback (DCI 5): no TD for 38.2 ms;
  - playback (DCI 2): no TD for 38.7 ms;
  - both said `WAITING ON A COMMAND FOR 8.0 MS` (8.3 MS for playback): `usb_host` was in a command of capture's close;
  - `audio_gw`'s calls were about 32 ms before and 30 ms after, while it closed capture;
  - so they belong to one close, the order 000-DRV-0081 is about, not to the running streams.
- **Against run A** (6 and 6 in 15 minutes, the first at a log read over the cable), none came while the streams ran with nobody on the cable.
  - That fits the guess that the cable's transfers starved the rings, but does not show it: no dry ring fell in a transfer's wait.
  - A run with a log read in its middle would name the transfer.
- **Criterion 3 is not met:** DCI 5 ran dry once at the close, and steps 2 and 3 are not written. All of this holds for that machine, that board and that release (MC-12.1).

- **Step 2, written:**
  - An isochronous IN stream has a spill of 64 KiB, more than half a second of the speakerphone's capture. Its finished TDs go into it as a bulk stream's do, and back on the ring at once.
  - `usb_host` does this every STREAM_MS, and now also every millisecond while it waits for a transfer of another device (`completion_for`). So a client that comes late, or a transfer of the cable's converter, no longer leaves the ring empty. The same holds for the cameras' bulk rings (158-DRV-0082).
  - Empty packets are left out of the spill, as the take leaves them out. A full spill leaves the packets on their TDs, so the ring can still run dry then, and says so.
  - The end line adds the spill's most and how often it was full.
  - Command waits are left as they were: a command may change the slot's endpoints.
  - QEMU's `usb-audio` has no IN endpoint, so the QEMU suites do not reach this path. The Ryzen run is its evidence.
- **Runs C and D on the Ryzen, 2026-11-22.**
  - The setup: release 202611220957; the board behind the Genesys hub; `audiostub duplex 300 0`, each with one log read over the cable at its middle; nobody else on the cable.
  - The raw lines are in `S/handoff/cam/0094-runs-C-D-and-cameras.txt`; the bench log has each run's times.
  - **C, step 1's `usb_host`** (duplex 10:23:42–10:28:45 UTC, the read 10:26:30–36):
    - all 64 dry rings fell inside the read, each `WAITING ON SLOT 3 ENDPOINT 4 FOR 30.7–45.4 MS`, the cable converter's (CH340) bulk OUT carrying the log;
    - capture 25, feedback 22, playback 20 (3 at the close);
    - so `usb_host` waits 30–45 ms on each of the converter's writes, serves no take meanwhile, and each ring's 32 ms of TDs runs out.
  - **D, step 2's `usb_host`** (10:30:12–10:35:14, the read 10:32:57–10:33:06):
    - capture 0 times dry (its spill at most 4508 bytes, never full) and feedback 0 times dry, its close too;
    - playback, an OUT ring, still ran dry 12 times in the read, all waiting on the converter, and once at the close. That is step 4.
  - It holds for that machine, that board and that release (MC-12.1).

## Related

- [000-DRV-0073](000-DRV-0073-usb-audio-class-speakerphone.md): the speakerphone in `audio_gw`. Run A is its evidence.
- [000-DRV-0093](000-DRV-0093-a-duplex-audio-stub-for-the-speakerphones-runs.md): the duplex stub and run A.
- [158-DRV-0082](158-DRV-0082-a-stream-for-each-camera.md): the bulk streams' spill, which step 2 follows.
