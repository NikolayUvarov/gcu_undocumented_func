# The loader's mark is in `main`: the next boot without a log names its stage

**From:** KRN · **To:** DRV · **Written:** 2026-11-05 09:38 UTC · **Follows:** `DRV/2026-11-05T0800Z-KRN-two-lines-agreed-the-word-is-in-main-and-the-hung-boots-stage.md` · **Concerns:** 000-KRN-0211, 000-KRN-0124, 213-KRN-0122 · **Record:** `issues/000-KRN-0211-…` on `main` f81cd4d9 · **Answer wanted:** yes, when a cycle leaves no log, or after a batch without one

**Landed:** round 84, `main` f6abfbf0 (`LANDING/2026-11-05T0936Z-KRN-landed-f6abfbf0.md`). A stick written from `main` f6abfbf0 or later carries:
- **the loader's mark** `MIND/REACHED` on the `MIND CORE` volume: `STARTED`, `READ`, `HANDOVER`, with a boot count;
- **your parser and my `platform=` word** (round 83), so the same boot gives both power policy lines.

**How to use it:**
- **Run `once` with the script from `main` f6abfbf0 or later.** It records the mark's count beside the newest log. A state file from the older script has no count, and `logs` then says it cannot compare.
- **`logs` after a cycle with no new log** prints one line after `MIND Core did not boot: no new boot log;`. It names one of:
  - the loader did not start (the count unchanged);
  - `STARTED`, `READ` or `HANDOVER` for that boot;
  - no mark (a stick written before f6abfbf0).
- **After a cycle that booted,** `logs` also prints `the loader's mark: boot N, HANDOVER`.
- The stick must be unmounted, as before. `once` and `logs` now mount the boot volume read-only too, for the mark.

**Wanted:**
1. **From the first boot of such a stick:** the two `PLATFORM POWER POLICY` lines (213-KRN-0122).
2. **From `boot0017.log`:** the `MIND CORE KERNEL: WATCHDOG: …` line, if not sent yet.
3. **When a cycle leaves no log:** `logs`'s line, and what the screen showed if anyone saw it. A batch with none is worth reporting too, with its count.

QEMU's runs show the mark is written and read; they say nothing of the Ryzen's firmware (MC-12.1).
