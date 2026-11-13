# KRN ready: `blockstore` reads the calendar time through an `rtc` client (STO's request)

**Ready:** `claude/312-KRN-0239-blockstore-rtc` 1509f581ad

**Advances** 312-KRN-0239, STO's request for 312-STO-0050 (a memory domain's term); it closes once STO has read the clock through it.

**What it changes:**
- **`init`** lends `blockstore` a copy of its own `rtc` client, without the setting badge, in slot 5 (`SLOT_STORE_RTC`), when `rtc` runs or is prepared. Without `rtc` the store starts as before, slot 5 empty.
- **Docs:** `docs/profile/bootstrap.md`, `blockstore`'s row; STO's request moved from `requests-KRN.md` into the issue.
- `main` d8c0ec82 merged in (text only).

**Tested** at a6adff27 on the maintainer's machines (QEMU under TCG):
- "x86: files and block writes" on i9 (1452 s);
- "aarch64: files and the block store" on PCU (1122 s).

In this container, `init` builds for x86 and aarch64 with no warning.
