# 000-DRV-0066 — `audio_gw`: the AC97 capture ring overflows in the `listen` suite on x86 TCG

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-10) · **Blocked by:** — · **Main task:** [000-APP-0114](000-APP-0114-hear-waits-for-a-whole-command.md) (the suite's check stays as it is) · **Constitution:** MC-6.1, MC-12.1

Numbered from the tools track's request of 2026-10-30 in `requests-DRV.md`.

## Problem

- **What fails.** The `listen` suite's 1 s recording reports `[LISTEN] RECORDED 48000 FRAMES (1000 MS), PEAK 0, RMS 0, OVERFLOWS 1`, where it requires `OVERFLOWS 0`. `audio_gw` logs `CAPTURE STOPPED: 1002 MS OF SOUND, PEAK 0, 1 OVERFLOWS`.
- **How often** (x86 QEMU, 4 CPUs, TCG, the tools track's container, 2026-10-30): 7 runs of 13. Among them:
  - 1 of 2 with `main`'s own `listen`, so 000-APP-0114's change is not the cause;
  - 1 of 4 with the check typing nothing while it records.
- **What was seen with it:** the compositor's frames came 5 to 10 a second, the longest up to 285 ms. The runs without an overflow had holds as long.
- **What it may be,** not shown: the ring is 16 buffers of 4 KiB (`CAPTURE_BUFFERS`), 341 ms of 48 kHz stereo. Under TCG the emulated card fills it at the host's pace, so a reader late by a third of a second overflows. Playback has 32 buffers.

## Plan

1. **Find the cause:** `audio_gw` logs, on an overflow, how long its reader had not taken a buffer and how full the ring was.
2. **Then the fix:** a capture ring as long as playback's (32 buffers, 683 ms), or a read that keeps up, whichever the log shows is needed.

## Acceptance criteria

- The `listen` suite passes 20 runs in a row on x86 TCG, with 4 CPUs, beside a build.
- The check's requirement, `OVERFLOWS 0`, is unchanged.

No real machine is needed.

## Progress (2026-11-10)

**Plan step 1, the cause, logged and seen.**
- **The log line:** on an overflow, `audio_gw` says how long since it last took from the DMA ring and since its reader last asked, and how much it kept: `[AUDIO] CAPTURE OVERFLOW: THE DMA RING (341 MS) FILLED; THE GATEWAY LAST TOOK FROM IT … MS AGO, ITS READER LAST ASKED … MS AGO, … OF … BUFFERS KEPT`.
- **The suite:** when the recording reports an overflow, it adds `audio_gw`'s log to the failure.
- **Seen on this machine** (x86 TCG, 4 CPUs, 24 host CPUs), with the gateway taking only when asked as before:
  - **10 runs alone:** no overflow.
  - **10 runs beside a build loop** (load average 65 to 75): the recording overflowed in 4.
  - **Each time** `listen` had not asked for 242 to 386 ms, and the gateway had taken nothing in that time.

  So the reader was late: the gateway emptied the ring only when its reader asked. Under that load the guest also runs behind QEMU's sound, which fills the ring at the host's pace: twice the ring filled in 242 ms of the guest's clock.

**Plan step 2, the fix.** The DMA ring is `init`'s region (`AUDIO_DMA_BYTES`, 16 capture buffers), so it stays as it is. Instead the gateway keeps up:
- **While the microphone records,** its loop looks at the DMA ring every `POLL_MS` (20 ms) and at each message. It takes the completed buffers into its own copy, `KEPT`.
- **The copy:** 256 buffers, 1 MiB, 5461 ms.
- **A read** gives the oldest kept buffers. The capture's start empties the copy.
- **An overflow** is now the DMA ring filling while the copy has no room, or while the gateway itself was not running for 341 ms.

The first copy of 64 buffers (1365 ms) still overflowed once in 10 runs beside the build. The copy was full there: the reader had asked 75 ms before, but took 4 buffers a call while the sound came faster than the guest ran.

**The criterion.** 20 runs of the `listen` suite, x86 TCG, 4 CPUs, beside the same build loop (load average 63 to 76):
- **The recording:** no overflow in any of the 20. Before the fix, 4 overflows in 10 runs under that load.
- **The suite whole:** it passed in 9 of the 20.
- **The other 11 failed after the recording:**
  - the suite's waits for the shell's answer to `hear --wav nosuch.wav`, `voice on --wav`, `hear x y` and `run keys`;
  - three times `HEAR: THE MICROPHONE IS BUSY`, which depends on `listen` still recording when `hear` asks.

  The same failures came with the old gateway under that load (3 of its 10 runs). They are the load's on the suite's timing, not the capture's.

So "the `listen` suite passes 20 runs in a row beside a build" is met for the recording, not for the suite's later checks under this load. The issue stays open until the whole suite holds, or its criterion is narrowed to the recording, which is APP's and the maintainer's to decide.

## Related

[000-APP-0114](000-APP-0114-hear-waits-for-a-whole-command.md), [000-DRV-0057](000-DRV-0057-the-output-volume-and-the-microphones-gain.md).
