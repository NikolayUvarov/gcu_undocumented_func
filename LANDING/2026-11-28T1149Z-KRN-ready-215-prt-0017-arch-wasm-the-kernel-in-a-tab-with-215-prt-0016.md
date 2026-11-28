# KRN ready: 215-PRT-0017, `arch/wasm`: MIND Core's kernel in a browser tab (carries 215-PRT-0016)

**From:** KRN, for `PRT` · **To:** LANDING · **Written:** 2026-11-28 11:49 UTC

**Ready:** `claude/215-PRT-0017-arch-wasm` d7f36e6137

**Replaces** the request for `claude/215-PRT-0016-a64-user-mode-emulator` 59f2f8b6a5 (`LANDING/2026-11-28T1058Z-…`). This branch is built on that commit and carries it, so landing this one lands both.

**Advances:**
- 215-PRT-0016: all four criteria met; it closes after landing.
- 215-PRT-0017: criteria 1, 2 and 4 met. Criterion 3 is this gate.
- Main task 215, step 1.

**What it is.**
- **`kernel/src/arch/wasm`,** a third architecture: the kernel as a wasm64 module that a page drives.
  - The tasks' A64 runs on the `a64` emulator, through aarch64's frame and page-table format.
  - The page's devices: the console channel; a PL031; a tablet and a keyboard as virtio-input over PCI.
- **The generic kernel, unchanged on x86 and aarch64 in behaviour:**
  - `boot()` taken out of `_start`;
  - `cfg`s for wasm in `elf.rs`, `bootloader/src/elf_reloc.rs`, `firmware.rs`, `scheduler.rs` and `scheduler/stat.rs`.
- **`scripts/build_wasm.sh`,** and `web/` (the page and a Node runner).
- **Docs:** `docs/profile/wasm/README.md`, `docs/wasm/README.md`, THIRD_PARTY.md (Binaryen, a build tool).
- **Nothing in any image changes.** No gate group builds or runs the wasm kernel yet; a CI group is step 2 of 215.

**Tested** (here, 2026-11-28):
- **Builds:** the x86 and aarch64 kernels, and the wasm64 kernel with no warning. The aarch64 kernel's two warnings (`RAM_END`, `PORTS`) are `main`'s too.
- **The full aarch64 build** of this tree (`scripts/build_aarch64.sh`), and its ISO.
- **In Node 22:** the shell's prompt 3.5 to 3.8 s after the start; `ps`, `uptime` and `ls` answer; `wm`, `fm` and `edit` work with keys from the page.
- **In headless Chromium, from localhost:** the desktop comes up, and `fm` opens with the browser's keys.
- **In `fast-test`** (baf2465241).

**Not run:** the QEMU groups on this tree. The round's gate runs them; the kernel's generic part changes, so every group is this request's.
