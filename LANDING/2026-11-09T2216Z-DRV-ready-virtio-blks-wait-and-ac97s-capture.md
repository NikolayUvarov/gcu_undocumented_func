# DRV → LANDING: ready, `virtio_blk` waits against the clock (000-DRV-0065, closed) and `audio_gw` keeps AC'97's capture (000-DRV-0066)

**Ready:** `claude/ASR-DRV` ad72bf69

**On `main`** fe1d3cd6, merged in; it brought only APP's voice scripts and issue text.

## What it changes

- **5fc17b69, 000-DRV-0065** (closed):
  - **The fault:** `virtio_blk` polled the used ring about 1,000 times, then slept 1 ms between polls. A sleep is a whole tick (10 ms on aarch64), and under TCG almost no request ended within the spins.
  - **The fix:** it polls for 2 ms against the monotonic clock, reading the clock every 64 polls, and only then sleeps. A request fails at a 10 s deadline, where before a count of polls gave about 2 s on x86 and 20 s on aarch64.
  - **Files:** `virtio_blk/src/wait.rs`, and the host test `tests/virtio_blk_wait_host.rs`.
- **310bec93, 000-DRV-0066** (stays open):
  - **The fault:** `audio_gw` took AC'97's capture from its 16-buffer DMA ring (341 ms) only at the reader's `record_read`. Beside a build loop, `listen` went 242–386 ms without asking, and the ring filled.
  - **The fix:** while recording, the gateway moves finished buffers every 20 ms, and at each message, into its own copy of 256 buffers (about 5.5 s).
  - **The log:** an overflow says how long since the gateway took and since the reader asked. The listen suite adds `audio_gw`'s log to a failing recording.
- **31234599:** the two branches' merge.

## What was tested, where

On this machine:
- **5fc17b69:**
  - the host test, 4 of 4;
  - aarch64 `net`: 35 of 35 on its second run. The first stopped at NET's static setup check, a race in that test (told to NET);
  - `aarch64: files and the block store`: PASS;
  - `x86: files and block writes`: PASS on a quiet machine. A first run timed out in `edit`, which uses the `ata` disk, at a host load of 45;
  - the whole `scripts/host_tests.sh`: 198 suites, none failed.

  The download's 8 MiB read back went from 5,120 ms to 400–540 ms on aarch64.
- **310bec93:**
  - `./02_build.sh`;
  - `x86: build (x86 test programs)` and `x86: services, storage, audio`: PASS;
  - the whole `host_tests.sh`: 197 suites, none failed;
  - 20 `listen` runs beside a build loop (load 63–76): the recording never overflowed, where the old code overflowed in 4 of 10.
- **After the merge:** both arches built, and the `virtio_blk_wait` host test passed again.
