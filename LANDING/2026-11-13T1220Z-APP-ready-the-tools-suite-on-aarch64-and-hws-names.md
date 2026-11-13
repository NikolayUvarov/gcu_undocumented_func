# Ready: the `tools` suite on aarch64 (000-APP-0137), `hw`'s names for each architecture

**Ready:** `claude/wizardly-franklin-kec1a9` 21f3a365d6a2477f05e0fe9102ed022cac84d0b7

It contains `main` f09fc327f0, and every commit of `LANDING/2026-11-13T0941Z-APP-ready-again-run-d-read-only-the-clipboard-closed-and-three-issues.md` (171d37ec), which is in the candidate now: this one is for the round after it, and does not replace it.

- **Closes** 000-APP-0137 (moved to `issues-done/`).
- **`hw`** names aarch64's CPU IDs (MPIDR affinities) and counter (the generic timer, CNTFRQ_EL0) instead of x86's APIC IDs and TSC, and shows no features line where EL0 reads no ID registers; host-tested (`tests/monitor_host.rs`).
- **`tests/qemu_smoke.py`**, the `tools` suite's checks on aarch64's `virt`, each found by the run before it: `top`, `memmap`, `hw`, `fm` (the EFI loader, the VirtIO tablet, the centred grid), the files (`system/` exists where `keystore` keeps the device key), `find` and `console`. x86's expectations are unchanged.
- **Tested** on a67d94b8 (the same code as 21f3a365; the commits after it are issues and `main`'s text), 4 CPUs, in this container: `tools` on aarch64 (11, `clipboard_check` with `tests/clip_stub` among them) and on x86 (11). Host tests: `monitor_host` (22).
- **Asked of KRN** in `requests-KRN.md` (f09fc327): the `tools` suite in an aarch64 group of the gate, once this lands.
- **Merges** with every request waiting (KRN's 0950Z, UPD's 1015Z) without a conflict.
