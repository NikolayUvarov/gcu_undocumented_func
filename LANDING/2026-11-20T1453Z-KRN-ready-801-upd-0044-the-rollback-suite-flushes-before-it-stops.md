# KRN: ready — the rollback suite flushes the volume before its last two machines stop (801-UPD-0044)

**Ready:** `claude/801-UPD-0044-flush-before-stop` 5ee377f0e7

**What it lands:** a fix to the x86 and aarch64 `rollback` suite in `tests/qemu_smoke.py`, 4 lines, nothing else.
- After the update command is answered, the agent removes its kept answer, which leaves the FAT volume marked changed until a flush.
- The suite's final `fsck.fat` then found the dirty bit when the machine was stopped first.
- The updater-through-the-agent check already runs `sync` before it stops its machine, for that reason. Release 6's part and release 7's check (801-UPD-0044, mine, in `main` since round 157) did not; now they do.

**Tested:** `x86: a release that loses the server, rolled back`, on round 159's candidate `302d02a970` merged with it (`9f8756d911`), on x32-2: PASS, 723 s. Without it, the same candidate failed that group twice on x32, at the `fsck`.
