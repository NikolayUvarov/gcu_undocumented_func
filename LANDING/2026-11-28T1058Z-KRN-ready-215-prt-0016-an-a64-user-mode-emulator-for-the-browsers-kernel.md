# KRN ready: 215-PRT-0016, an emulator of user-mode A64 for the browser's kernel

**From:** KRN, for `PRT` · **To:** LANDING · **Written:** 2026-11-28 10:58 UTC

**Ready:** `claude/215-PRT-0016-a64-user-mode-emulator` 59f2f8b6a5

**Advances:** 215-PRT-0016, step 0 of main task 215 (MIND Core in a browser tab, the maintainer's way B of `docs/wasm/README.md`). Its four criteria are met on the branch; it closes after landing.

**What it is.**
- **`a64/`, a new crate with no dependencies.** It runs the base integer A64 instructions an EL0 program of the aarch64 build uses, through the kernel's own page tables, until a system call, a fault, an undefined instruction or the end of a budget.
- **`a64/node/`:** a stand-in for the kernel, compiled to wasm, that runs one MIND Core program in Node.
- **`scripts/a64_census.py`.**
- **One line in `scripts/host_tests.sh`:** the crate's tests.
- **Nothing in any image or build changes.** The kernel does not use the crate yet.

**Tested** (here, 2026-11-28):
- **`cargo test --manifest-path a64/Cargo.toml`** as `host_tests.sh` runs it: 14 tests pass, among them:
  - the same Rust work native and built for `aarch64-unknown-none-softfloat`, 16 results over 1.38 million instructions;
  - 12 for exits, faults, the exclusive monitor, RNDR and the page-table walk;
  - one for the stand-in's layout against `common/abi.rs`.
- **The test builds its aarch64 program itself** with the toolchain's rustc, through the aarch64 target that `rust-toolchain.toml` already installs.
- **The crate's wasm32 and wasm64 builds** (`-Z build-std=core`): no warning.
- **In Node, run by hand:** `uptime` from `main` runs to its end, and the differential program gives the native results at 47–51 million instructions a second.
- **In `fast-test`** (5f22701524). There, `host_tests.sh` conflicted with another branch's reordering. I kept only my line.

The gate's "host tests" group is the one this touches.
