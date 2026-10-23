# 351-KRN-0166 — The loader's handoff versioned apart from the system-call ABI

**Type:** kernel (`main.rs`), `common/abi.rs`, bootloader · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-12.4, MC-12.7, MC-6.5

From EFF's request of 2026-10-23 (`internal-dev-messages`, `KRN/2026-10-23T1856Z-EFF-two-requests-a-slot-update-across-a-syscall-abi-change.md`), filed at the maintainer's word.

## Problem

- **One number covers two interfaces.** `BootInfo.abi_version` carries the system-call ABI (`ABI_VERSION`). The kernel compares it with its own at its first instruction and stops on any difference (`kernel/src/main.rs`).
- **Two ABIs that left the handoff alone still needed a new stick.**
  - ABI 18 added `WORKER_START`; ABI 19, large pages and `ALLOC_SMALL_PAGES`. Neither changed `BootInfo`.
  - Yet a kernel of either refuses the ABI 17 loader.
  - A machine whose `BOOTX64.EFI` lies outside its slots therefore cannot take such a kernel through a slot update; the MacBook Pro over the link is one.
- **Programs already check the right thing.** `libmind` compares a program's ABI with the kernel's (`libmind/src/process.rs`). The loader's check is the one that is too wide.

## Plan

1. **Two versions in `common/abi.rs`.**
   - `ABI_VERSION`, as now, between programs and the kernel.
   - `BOOT_HANDOFF_VERSION`, for `BootInfo`'s layout and meaning between the loader and the kernel.
2. **The field at offset 720 holds the handoff version.**
   - The loader writes `BOOT_HANDOFF_VERSION` there.
   - The kernel accepts the versions whose `BootInfo` it reads as its own: from the ABI at which `BootInfo` last changed up to `BOOT_HANDOFF_VERSION`.
   - Older loaders wrote their ABI there, so an ABI 17 loader is accepted when `BootInfo` has not changed since 17. Which ABI that is, is found by comparing `BootInfo` across the ABI notes in `common/abi.rs`.
3. **A change to `BootInfo` raises `BOOT_HANDOFF_VERSION`** and moves the lowest accepted version to it. That is the explicit transition of MC-12.4: a kernel and a loader of different handoffs refuse each other, with the message they give now.
4. **A host test pins the layout**: `BootInfo`'s size and its fields' offsets. A layout change without a new handoff version fails it.

## Acceptance criteria

- [ ] A kernel of ABI 19 boots from the loader of an earlier ABI whose `BootInfo` is the same (QEMU, x86 and aarch64), and programs of ABI 19 run.
- [ ] A loader of another handoff is refused with the message, as now, by the test kernel the `loader-abi-test` feature builds.
- [ ] Host tests: the layout pinned; the accepted range named in one place.
- [ ] `docs/api` and `docs/profile` state the two versions and which change needs a new loader.

## Related

[351-KRN-0167](351-KRN-0167-a-kernel-that-refuses-its-loader-on-trial-restarts.md), 351-UPD-0010 ([updating the bootloader](351-UPD-0010-updating-the-bootloader.md)), [351-KRN-0141](351-KRN-0141-an-update-through-the-link.md), [351](351-self-update.md).
