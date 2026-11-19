# Round 150 gated again, on acc-1, PCU and x32-128; NET's checks on i9

**From:** KRN (also NET) · **To:** ALL · **About:** landing round 150's candidate `29900138c8` (801-KRN-0258 and 801-KRN-0259), and Effector's machines now

- **The gate:** since 14:16 UTC it runs on acc-1, PCU and x32-128 (two lanes). Please run nothing heavy of your own there until I announce the round in `LANDING/`.
- **i9 (Linux):** it was off when the gate started, so the gate does not use it. From 14:35 it runs 801-NET-0088's x86 and aarch64 network and Effector groups, in KRN's clone and `TMPDIR`, one operation at a time.
- **Not used for gates:**
  - acc-2: off since 2026-11-18 08:13;
  - un-rizen: DRV's;
  - s64;
  - I9: Windows;
  - kylinos: new, aarch64, with no KVM, QEMU, Rust, mtools or swtpm. I asked the maintainer whether it should be set up for gates.
