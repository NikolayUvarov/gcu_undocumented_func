# Ready: the loader's mark of how far a boot got, read by `boot_switch.py logs` (000-KRN-0211)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-05 08:37 UTC · **Concerns:** 000-KRN-0211 (advanced), 000-KRN-0124 (advanced)

**Ready:** `claude/youthful-mendel-mf1soy` f6abfbf029124d7655df3fae5b51fd308c54d2ae

It contains `main` as of now (5694ea03).

## What it brings

- **The loader's mark, `MIND/REACHED`,** on a volume with slots:
  - `STARTED` with a boot count;
  - `READ` once the kernel and services are checked;
  - `HANDOVER` before boot services end.

  A slot's own loader keeps the count. A failed write is named on the serial line and never stops the boot.
- **`boot_switch.py`:** `once` records the count. `logs`, for a boot that left no log, says whether the loader did not start or which point it reached.
- **Docs:** `docs/update/slots.md` and `docs/write-disk.md`, English and Russian.
- **Why:** two `BootNext` boots on the Ryzen kept a dark screen and left no log (000-KRN-0124).

## Tested, on f6abfbf0's code (eb5879e0; the later commits change only the issue)

- **Host:** `tests/boot_mark_host.rs` and `tests/boot_switch_test.py`, in this session's container; `host tests` on x32-128 (752 s).
- **QEMU through Effector,** all passed:
  - acc-1, x86: `boot, display, shell, memory, clock`, `USB image`, `Secure Boot with our keys`, `NVMe boot disk`;
  - x32-128, x86: `the updater and an update through the link`, `a developer's release through the Effector agent`, `reproducible build`;
  - acc-2, aarch64: `boot and fault containment`, `the updater and the modes`, `a developer's release through the Effector agent`, `an update through the link`, `NVMe boot disk`.
- **The new checks** (`boot_slots_check.py`) passed on both architectures:
  - three marks per boot, one count more each boot;
  - a reset in a slot loader's pause left at `READ`;
  - `fsck.fat` clean after.
