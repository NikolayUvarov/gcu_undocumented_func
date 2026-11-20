# KRN: ready — a trial the watchdog restarted with no mark of init's is reported, and a fallback is taken once (801-UPD-0046)

**Ready:** `claude/801-UPD-0046-a-trial-reset-with-no-mark` eed9a278c6

**What it lands:** 801-UPD-0046 (closes once landed).
- **`updater`:**
  - **The fallback is taken once.** At the first boot of the slot a trial fell back to, once `init` answered how it ended, the updater keeps the newer record's sequence in `system/updater/fell-back`. A later boot with the same record does not ask again.
  - **A watchdog restart with no mark of `init`'s,** at that first boot, is the trial's, and is kept as its refusal.
- **`tests/qemu_smoke.py`:**
  - a `watchdog` suite check with the `watchdog-test` kernel, which stops before `init`;
  - 801-UPD-0044's and 0045's later-boot checks expect the taken-once line.
- **Docs:** docs/update/updater.md 0.18 (EN and RU), docs/profile.
- **No IDL or ABI change.**

**Tested** on `f94d1f3fb4` (`main` 28b5b8dc22 merged), each PASS:
- i9: `x86: boot, display, shell, memory, clock`, 1770 s;
- x32: `x86: the watchdog`, 512 s, and `x86: a release that loses the server, rolled back`, 727 s;
- x32-2: `x86: the updater and an update through the link`, 1373 s;
- acc-1: `aarch64: the updater and the modes`, 1169 s, and `aarch64: a release that loses the server, rolled back`, 711 s;
- here (QEMU, x86): the three checks of a fallback slot's later boot, each PASS.
