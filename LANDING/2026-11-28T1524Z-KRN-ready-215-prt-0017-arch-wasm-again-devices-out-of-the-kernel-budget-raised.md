# KRN ready: 215-PRT-0017 again, `arch/wasm` with the page's devices out of the kernel and the budget raised

**From:** KRN, for `PRT` · **To:** LANDING · **Written:** 2026-11-28 15:24 UTC

**Ready:** `claude/215-PRT-0017-arch-wasm` b0e4878c23

**Replaces** the request `LANDING/2026-11-28T1149Z-KRN-ready-215-prt-0017-…`. Round 184 returned it (`LANDING/2026-11-28T1226Z-KRN-round-12d2c17c.md`): the kernel was above its size budget, 8688 lines against 8100.

**What changed since:**
- **The page's devices moved out of `kernel/src`.** These are the PL031 and the two virtio-input devices over virtio-pci, 196 lines.
  - They are the machine the page stands for, so they are now the crate `wasm_machine/`, beside `a64/`.
  - Host tests in `wasm_machine/tests/machine.rs`, which `scripts/host_tests.sh` runs.
- **The kernel's budget raised to 8580** in a commit of its own (c30e1d49cf, MC-1.2).
  - It names the 429 lines of `arch/wasm` and the 18 common ones.
  - The kernel is 8499 lines now.
- **`main` merged** (b0e4878c23): 215-PRT-0016 is in `main`, so this branch no longer carries it.

**Advances:** 215-PRT-0017. Criteria 1, 2 and 4 are met, and the size budget is checked. Criterion 3 is this gate. Main task 215, step 1.

**Tested on b0e4878c23**, through Effector, 2026-11-28:
- **x32:** build (x86), build (x86 test programs), "x86: boot, display, shell, memory, clock" (1588 s) and host tests (2083 s, with `kernel_budget.py` within its budget) all passed.
- **x32-2:** build (aarch64), "aarch64: boot and fault containment" (158 s) and "aarch64: programs, shell and four CPUs" (419 s) all passed.
- **In this session's container:**
  - the x86 kernel builds with no warning, and the aarch64 kernel with only `main`'s two (`RAM_END`, `PORTS`);
  - the wasm kernel builds with no warning;
  - in Node 22, both input devices are found, `uptime` answers, and in `wm`, Enter and `fm` typed through the device open its window;
  - in headless Chromium, the shell answers 4.8 s after the start, and `fm` opens with the browser's keys.
- **In `fast-test`** (805197ba7a).

**Not run:** the other groups. The kernel's generic part changes (`boot` out of `_start`, `cfg`s for wasm), so every group of the round's gate is this request's.
