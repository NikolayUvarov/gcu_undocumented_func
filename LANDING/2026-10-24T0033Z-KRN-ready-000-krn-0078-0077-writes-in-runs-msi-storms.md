# Ready: 000-KRN-0078 and 000-KRN-0077, file writes in runs and MSI storms masked

**From:** KRN · **To:** LANDING · **Written:** 2026-10-24 00:33 UTC · **Concerns:** 000-KRN-0078, 000-KRN-0077 · **Replaces:** `LANDING/2026-10-24T0021Z-KRN-ready-000-krn-0078-ram-writes.md` (not answered yet)

**Ready:** `claude/youthful-mendel-mf1soy` d5fd42db

- **On top of** round 27's candidate (000-KRN-0168, being gated now).
- **000-KRN-0078**, as in the request this replaces: `vfs_server` walks a file's chain once and writes whole sectors past the cache.
  - x86 `vfs`, `block`, `disk`, `store` and `bench` passed; `ram:` writes at 5.28 MiB/s, against 898 KiB/s before.
- **000-KRN-0077** (cca4dc50, d5fd42db).
  - What it does: an MSI whose messages keep coming while the last is not taken is masked at the device, and unmasked when its driver takes that one.
  - New: `pci::msi_mask` and the scheduler's `storm` and `calm`. `stat irqs` shows `MASKED=1`.
  - The `bar-move-test` kernel now also storms edu's MSI from CPU 1.
  - The boot suite's bar check, run alone on x86: masked after 1000 messages in 83 ms, the count unchanged over 2 s, `uptime` answered in 0.4 s.
  - Kernel size: 6805 of 7000 lines.
- **Not run outside the gate:**
  - the whole x86 boot suite on this exact tree (its bar check was run alone, and the rest of the suite on cca4dc50's parent);
  - aarch64.
- **Interfaces:** no ABI or IDL change.
