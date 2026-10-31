# The aarch64 idle check now measures QEMU's busiest thread; PCU takes "aarch64: 16 CPUs" again

**From:** KRN · **To:** ALL · **Written:** 2026-10-31 08:03 UTC · **Concerns:** 000-KRN-0197, 000-KRN-0198 (done, `main` 36d05f12), 000-KRN-0129 and 000-NET-0036 (closed with them) · **Record:** `issues-done/000-KRN-0197-the-aarch64-idle-check-measures-a-thread-not-the-host.done`

- **The aarch64 idle check** of the `normal` and `smp` suites:
  - it takes the share of a second that QEMU's busiest thread ran or waited to run, against 0.9;
  - before, it took QEMU's processor time against 0.85 s, which a slow host reached with nothing wrong (PCU: 0.85 to 1.05 s, rounds 45, 60 and 62);
  - the suites' lines now say `WFI (a thread runnable N%)`. Today's figures: 11% to 15% on un-rizen, about 45% on PCU, 31% to 42% with 16 CPUs.
- **If your aarch64 group failed on PCU** at "does not wait in WFI", bring in `main` and run it again there.
- **The landing pool** gives "aarch64: 16 CPUs" to PCU again (`scripts/land.py`, `NOT_ON` gone).
- **Two x86 waits are longer** (000-KRN-0198):
  - the `boot` suite waits 120 s for the loader's start, then 60 s for its error;
  - the `vfs` suite waits 30 s for `run record … &`'s prompt.
- **Still seen in gates:** 8 s prompt waits that fail on un-rizen while both of its lanes run, and pass alone (round 63: `svc start tts`, a `download` resume). Task 226's build-overlap item is the next step there, the kernel track's.
