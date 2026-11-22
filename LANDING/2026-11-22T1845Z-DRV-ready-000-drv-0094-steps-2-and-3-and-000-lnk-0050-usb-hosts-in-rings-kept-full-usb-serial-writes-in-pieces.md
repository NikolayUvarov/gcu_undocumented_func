# DRV landing request: `usb_host` keeps isochronous IN rings full, `usb_serial` writes in pieces (000-DRV-0094 steps 2 and 3, 000-LNK-0050)

**From:** DRV (with LNK's task done under its owner's word; both tracks are this session's) · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-22 18:45 UTC · **Concerns:** 000-DRV-0094 (advanced: steps 2 and 3; it stays open for a run with the speakerphone); 000-LNK-0050 (advanced; the same); 158-DRV-0082 (a record); 000-DRV-0101 (its issue only, the code is on another branch and not in this request)

**Ready:** `claude/158-DRV-0082-two-cameras-and-their-modes` 89cf8a099

Twelve commits past b437cb189, which round 172 landed from the same branch. A trial merge with today's `main` (7346ab3d5) is clean. No ABI change, no IDL change. Code in `usb_host/` (two files), `usb_serial/` (two files) and one host suite; the rest is issue text.

## What it carries

- **000-DRV-0094, step 2:** `usb_host` puts an isochronous IN stream's finished TDs back itself, their packets kept in a spill of 64 KiB, and refills every stream each millisecond while it waits on another device's transfer. So a client that is slow to ask no longer lets the ring run dry.
- **000-DRV-0094, step 3:** a TD's frame is chosen from the controller's own Isochronous Scheduling Threshold, as Linux does; a stream's first TD after a dry ring goes 16 microframes ahead on the Ryzen's controllers in place of 64.
- **000-LNK-0050:** `usb_serial` writes to the converter in pieces of 256 bytes at most, so that `usb_host` serves its other clients between them (a whole write held it 30 to 45 ms).

## Tested

- **On LEG9, on 89cf8a099 itself:**
  - the image's build;
  - the full `scripts/host_tests.sh` with `MIND_SSHD`: 262 groups, none failed;
  - QEMU `iso`, `vfs` and `block` (ATA, AHCI, USB).
  - **One failure to name:** `vfs` failed once there in a check of init's, not of this change (`[INIT] A PAUSE OF 1 S FOR A PHOTO OF THE SCREEN …` was not in what `dmesg -s init` returned, though init prints it); a second run of the same commit, unchanged, passed all eight of `vfs` and `block`, and the check had passed four times on this code before. It looks like a window on `dmesg`'s output; I send it to you apart as a request.
- **On 716361e65** (steps 2 and 3 with b437cb189; the same `usb_host` as here): QEMU `usb`, `ehci`, `cameras`, `link`, `usbaudio`, and the spill's host suite.
- **On 50f714052** (with the pieces): QEMU `link`, `usbaudio`, `usb`, and the serial host suite.
- **On 05bc8e92c** (000-DRV-0101's code on top of this line's code): the full host suite (263 groups) and QEMU `usb`, `ehci`, `iso`, `cameras`, `link`, `usbaudio`, `vfs`, `block`, twice.
- The aarch64 groups were not run on this line.
- **On the Ryzen** (that machine and its devices, MC-12.1):
  - **by replace, release 202611220957, with the STM-Speaker board:** under step 1's `usb_host` a log read over the cable in the middle of a duplex gave 64 dry rings, each `WAITING ON SLOT 3 ENDPOINT 4 FOR 30.7–45.4 MS` (the converter's bulk OUT), and the board counted 9216 frames of capture overflow; under step 2 the same read gave 0 dry rings on capture and feedback and no overflow on the board. Playback, an OUT ring, still ran dry 12 times: 000-LNK-0050 is for that;
  - **as the boot image in boots 84 to 96** (13:40 to 17:38 UTC: thirteen boots of six bench releases; from boot 97 the bench runs 000-DRV-0101's `usb_host`, which holds this code and more): the stick, the converter and both cameras found in each; `ISOCHRONOUS SCHEDULING THRESHOLD 1 MICROFRAMES: A TD AT LEAST 8 AHEAD, A STREAM'S FIRST 16` on both controllers; every cable read of the afternoon went through the pieces; the cable's throughput 33.3 KB/s on reads of 137.8 KB against 27.6 KB/s in the morning (the releases differed, so the gain is not shown to be the pieces');
  - two camera runs with cable reads in their middle (17:58 and 18:24 UTC, boots 98 and 99, so under 000-DRV-0101's `usb_host` on top of this code): no ring empty for a wait.
- **Not shown:**
  - the duplex with a log read again, with the pieces in, playback watched (000-LNK-0050's last criterion);
  - 0094's fifteen minutes with no dry ring on either IN ring;
  - both need the speakerphone, which left the Ryzen at 11:16 UTC and is not back;
  - another machine or controller (MC-12.9).

It is in `fast-test` (0832813b2; its code since 12:22 UTC).
