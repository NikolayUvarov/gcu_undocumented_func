# 000-KRN-0067 — The aarch64 boot test passes on PCU

**Type:** CI · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track A (CI) · **Constitution:** MC-12.1

## Problem

On 2026-10-10 the maintainer decided that the aarch64 part of a gate runs on the remote machine PCU (`docs/effector.md`, section 4). Every aarch64 group passes there except "aarch64: boot and fault containment" (`tests/aarch64_smoke.py`):

- **The failure.** QEMU aborts within 8 s, as the guest writes its boot volume:
  - `block/vvfat.c:2760: handle_renames_and_mkdirs: Assertion 'j < s->mapping.next' failed`;
  - the boot volume is a `fat:rw:` directory;
  - the last lines are `[INIT] READY`, and `[BLOCKSTORE] READY` never comes.
- **What it is not.**
  - **The QEMU version.** QEMU 8.2.10 from qemu.org fails the same way, built in `~un/qemu-upstream/stage` and not installed.
  - **Ubuntu's patches.** Ubuntu's 8.2.2 (`8.2.2+ds-0ubuntu1.18`, on CI's runners and on the kernel session's machine), where the test passes, carries no `vvfat` patch: its `debian/patches/series` names none.
  - **The stale copy.** A suite run cut off that morning had left an old disk copy, `aarch64_root/smoke-*`, in the build, and the test copied it onto the disk. Without it, the test still fails.
- **What differs.** The host: Ubuntu 22.04 on ext4, a faster boot (8 s against about 18 s), AAVMF 2022.02. The guest's writes may come in another order or at other times there, and `vvfat`'s bookkeeping of renames and new directories is known to be fragile.

## Plan

1. Find which write breaks it. Run the test with QEMU's `-trace 'vvfat*'` on PCU, and compare with the kernel session's machine.
2. If `vvfat` cannot follow the guest's writes, give the test a FAT image instead of a `fat:rw:` directory, as `boot_slots_check` already does with `raw=True`. The test checks the system, not QEMU's directory emulation.
3. Keep leftovers of interrupted runs (`smoke-*`, `*.ppm`) out of the copied build (done on the kernel track's branch).

## Acceptance criteria

1. "aarch64: boot and fault containment" passes on PCU and on the kernel session's machine.
2. `docs/effector.md`, section 4, no longer sends that group back to an agent's own machine.

## Related

`docs/effector.md` (EN, RU) section 4; [176](176-test-and-performance-utilities.md) (the measurements that led to running aarch64 on PCU).
