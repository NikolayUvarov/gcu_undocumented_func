# 800-LNK-0033 — A machine busy writing its disk no longer ends a put: each try waits twice as long as the one before

**Type:** protocol, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-10-31) · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-12.1

**Asked by:** the maintainer (2026-10-31): "решай вопрос с обрывом кабеля".

## Problem

Two slot updates of the DEXP over the cable failed midway (2026-10-30 and 2026-10-31):
- **The failures:**
  - `effector485`'s `put` of a release's blob ended `failed: no answer from the other end`, at file 96 of 104 the first time (`voice.elf`, 785,056 bytes) and at file 57 the second (`console.elf`, 287,072 bytes);
  - a rerun with the same version went on, the files already there only checked.
- **The daemon's log** has the same line at times during long `log` reads too. The daemon then said hello again and went on.
- **The timing.** The failed put of 287,072 bytes ended 8 s after it began: about when its last data were sent at the line's rate, so at its close.

What the code shows:
- **The master's wait** (`linkproto::Master::exchange`): every try waited the same `timeout_ms`, from the rate: 200 ms plus 4 frames' time, 290 ms at 460,800 bit/s. 8 tries gave up after 2.3 s of silence.
- **`link` on MIND Core is silent while it writes its disk:**
  - at `PUT_CLOSE` it writes the last of its 16 KiB buffer, `flush`es the file to the USB stick, removes the old one and renames;
  - each `PUT_DATA` that fills the buffer writes it.

  A USB stick can take seconds for such a write.
- **Only `STAT` had a long wait** (15 s), for the slave's hash of the whole file.

## Plan

1. **In `Master::exchange`,** each try waits twice as long as the one before, up to `WAIT_MAX_MS` (4 s), or `timeout_ms` when that is longer (as `STAT`'s 15 s).
   - At 460,800 bit/s: 290, 580, 1160 and 2320 ms, then 4 s each. 8 tries ride out about 20 s of silence.
   - A lost frame is still found by the first, short tries.
2. **A host test:** a slave whose commit is silent 6 s keeps the put; one silent 60 s is given up on. The dead line's test takes the new waits.
3. **`docs/link/mindlink.md` (EN, RU):** `--timeout` and `--tries` say so.
4. **A new `effector485` for the maintainer's Windows machine** carries it, with 800-LNK-0032's `key` and `type`.

## Acceptance criteria

- [x] `linkproto`'s and `tools/mindlink`'s tests pass, the new one among them.
- [x] The QEMU `link` suite passes on x86.
- [ ] The daemon on LEG9 runs the new build. A slot update of the DEXP over the cable ends without a lost put.

## Related

[800-LNK-0011](800-LNK-0011-effector485.md) (the daemon), [800-LNK-0032](800-LNK-0032-keys-and-the-pointer-over-the-link.md), [docs/link/mindlink.md](../docs/link/mindlink.md).
