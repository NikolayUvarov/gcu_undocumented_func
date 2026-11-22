# 000-DRV-0094 — `usb_host`: an isochronous IN ring does not run dry while its client is slow

**Type:** driver (`usb_host`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-22: steps 1 and 2 written and run on the Ryzen; step 3 written; step 4 as 000-LNK-0050, written) · **Blocked by:** — · **Roadmap:** A · **Constitution:** MC-6.1, MC-11.4, MC-12.1, MC-12.2

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

4. **An OUT stream's ring kept full while `usb_host` waits for another transfer** (found by run D). The owner session's decision, 2026-11-22:
   - **Now:** the cable's converter, whose writes were the waits, writes in pieces of 256 bytes (000-LNK-0050). `usb_host` then serves `audio_gw`'s puts between two.
   - **Later, as its own task:** `usb_host` starts bulk transfers and answers from its loop instead of waiting on them (000-DRV-0101).
   - **Not done:** OUT packets held ahead in `usb_host`, since that delays playback by the queue.

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
- **Step 3, written** (not run on the Ryzen yet):
  - `usb_host` reads each controller's Isochronous Scheduling Threshold (HCSPARAMS2's IST: microframes, or frames with bit 3) and says it at its start (`ISOCHRONOUS SCHEDULING THRESHOLD n MICROFRAMES: …`).
  - A TD's frame must then be at least IST + 1 microframe ahead, in whole frames, as Linux's `xhci_get_isoc_frame_id` schedules. A stream's first TD, and the first after a dry ring, goes one frame further. Before, it went 64 microframes ahead (`FIRST_AHEAD`, a guess), and a TD continued down to 16 ahead.
  - On QEMU (IST 7 frames) that is 64 and 72. On the Ryzen it is to be read.
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

## For a successor (state on 2026-11-22)

The CAM agent worked here for the DRV owner session, on 158-DRV-0082 to 0084, 000-DRV-0093, 0094, 000-LNK-0050 and 000-DRV-0101. Its brief was `S/handoff/BENCH.md`.

**Branches and heads**
- `claude/158-DRV-0082-two-cameras-and-their-modes`:
  - **b437cb189** is in the landing queue (the request posted by the owner session, waiting for round 172). It holds:
    - 0082's spill, the stale cameras, the frame-break causes and the error code;
    - 0083's comparison with Linux; 0093's audio stub;
    - 0094 step 1;
    - 0084's IJG record;
    - 158-ASR-0016.
  - **43686b6a2** and the text after it are not requested yet. They hold:
    - 0094 steps 2 and 3;
    - 000-LNK-0050;
    - the 000-DRV-0101 issue.
  - 43686b6a2 is in `fast-test` and in the bench tree, and goes out in the release of 2026-11-22 13:35 UTC.
- **In `main`:** 502d9a4f2 landed in round 169 (0082 to 0084's core: a stream for each camera, the modes, `video.wit` 1.1, the JPEG decoder).
- `claude/000-DRV-0101-usb-host-answers-bulk-from-its-loop`, from 43686b6a2: 000-DRV-0101's work.

**How a Ryzen run is made** (`S` is the owner session's scratchpad; `S/handoff/bench-log.txt` is the bench log):
1. **The owner session's word first**, and a bench-log gap: the maintainer and the other agents share the machine.
2. **The lease:** `effector485.exe reserve --minutes ≤60 --reason … --owner CAM`; `renew` before it ends; `release` after. A bench-log line at each take and release, with the release (`MIND Core …`).
3. **A replace of `usb_host`** takes the keyboard, the cable, the stick and the Effector agent's journal away for some seconds.
   - Before it: a bench-log line saying that NOBODY SENDS A SERVER COMMAND until CAM writes that `usb_host` is back.
   - After it: read the log for `EFFECTOR: … REFUSED UNJOURNALED` and `THE JOURNAL CANNOT BE WRITTEN`, and tell the owner session at once if either is there.
   - The commands: `replace unit=usb_host file=<ELF in mindlink/tmp-upd>`; back with `replace unit=usb_host back=true`, and `video_gw` the same way.
4. **The stubs** live in `data/link/` (`put file=… name=videostub.elf`, `audiostub.elf`).
   - `audiostub duplex SECONDS 0` runs by the cable's `run name=audiostub.elf args=…`.
   - `videostub` needs a camera lent by the shell, so it is typed on the console: `type text="run data/link/videostub.elf 1 45 &"`, then `key names=ret`.
   - **Before any `type`:** the log's last `[COMPOSITOR] SCREEN OF PID n IN FRONT` must name the shell's PID (`[INIT] STARTED shell PID=n`). If it does not, stop: the keys go to whatever is in front.
   - The cable's `shell` refuses `run … &`; the cable's `run` lends no camera.
5. **Times:** STARTED, ENDED and each log read go into the bench log in UTC. The STM-Speaker agent holds its board's counters against them.
6. **No log read over the cable during a measurement,** unless the read is the measurement: a read is 30–45 ms waits on the CH340 for `usb_host` (run C).

**The helpers**
- Kept in `/home/un/.local/share/mind-bench/cam/`, which outlasts a restart of LEG9. They expect `/tmp/claude-1000/cam/` for the lease file, `runlogs/` and the worktrees.
- `e485.sh KIND [field=value…]`: one cable operation with a new key, `CAM_REASON` and the lease from `/tmp/claude-1000/cam/lease`, waited for (`CAM_TIMEOUT`).
- `ryzen-g2.sh cams|duplex|back`:
  - `cams`: the cameras' replaces and the typed YUY2 runs;
  - `duplex`: the 900 s duplex, with the bench-log lines;
  - `back`: both services put back, with the journal check.
- `ryzen-g3.sh C|D`: a 300 s duplex under step 1's or step 2's `usb_host`, with one timed log read at its middle.
- `eff/op.sh AGENT SCRIPT REASON TIMEOUT`: one Effector operation through `S/hes.sh` (the key stays there), for the aarch64 groups on x32-128. `eff/resume.sh` drives them group by group, quiet while a KRN gate runs.
- `tests/s4-run.sh` to `s9-run.sh`: the LEG9 runs per head, under `flock /tmp/claude-1000/heavy.lock`, which the DRV agents share:
  - `02_build.sh`, then `x86_fixtures.sh`;
  - `host_tests.sh` again until one run is whole: never cut down, nothing skipped;
  - `tests/qemu_smoke.py --suites …`.

**What is shown** (each for that machine, that board or camera and that release, MC-12.1):
- 0094 step 1's lines on the Ryzen.
- Run C: every dry ring named by the CH340's bulk OUT waits.
- Run D: step 2 keeps the IN rings full through a log read. The STM-Speaker board agrees: no capture overflow in D, against 9216 frames in C.
- The cameras' error bit nearly gone under step 2 (one run each, a guess).
- QEMU's IST is 56 microframes.

**What is open**
- Step 3's IST on the Ryzen: said at the 13:35 boot.
- 000-LNK-0050's timed reads after the pieces, and run D again with playback watched. These wait for the cable (COM22 lost on LEG9 at about 11:23 UTC) and the board (gone from the Ryzen at 11:16:42).
- This issue's criterion 3.
- 000-DRV-0081's close order.
- 000-DRV-0101.
- 0084's criterion 1: Ryzen MJPEG frames compared with Linux's decoding.
- The maintainer's answer on the IJG terms (`issues-human/README.md`, section 17).
- The Wi-Fi agent's hour of USB load, put off until the board and the cable are back.

## Related

- [000-DRV-0073](000-DRV-0073-usb-audio-class-speakerphone.md): the speakerphone in `audio_gw`. Run A is its evidence.
- [000-DRV-0093](000-DRV-0093-a-duplex-audio-stub-for-the-speakerphones-runs.md): the duplex stub and run A.
- [158-DRV-0082](158-DRV-0082-a-stream-for-each-camera.md): the bulk streams' spill, which step 2 follows.
