# 352-KRN-0294 — `block.wit` 2.1: the drive's model as it names itself, for the installer's disk list

**Type:** IDL (`idl/block.wit`), `libmind` (`mind::block::serve`), `vfs_server` (the installer) · **Owner:** `KRN` (the kernel session) · **Priority:** P3 · **Status:** open (2026-11-28) · **Blocked by:** — · **Main task:** [352](352-installable-media-for-pcs-and-arm-devices.md) · **Constitution:** MC-12.3, MC-12.4, MC-12.7

## Problem

`install.wit` 1.0's `disk` carries `model: string<40>`, so that a person can tell two drives of one size on one bus apart ([352-UPD-0048](352-UPD-0048-the-installer-onto-a-disk-of-this-machine.md)). `block.wit` 2.0 carries no identity, so `vfs_server/src/installer.rs` leaves `model` empty for every disk.

DRV accepted KRN's split on 2026-11-28 (`KRN/2026-11-28T0926Z-DRV-re-the-drives-model-the-split-suits-352-drv-0118.md` on `internal-dev-messages`). DRV's part is [352-DRV-0118](352-DRV-0118-the-drives-model-for-block-wit.md), blocked by this task.

## Plan

1. **`idl/block.wit` 2.1** (a minor version: one call added, 2.0's calls unchanged):
   `model: func() -> string<40>`: the drive's model as it names itself, printable ASCII with trailing spaces and NULs cut; empty when the driver keeps none. A windowed client sees its drive's model.
   - The generated files come from `scripts/mind_idl.py`, and the IDL fuzzer takes the new call.
2. **`mind::block`:**
   - `Driver::model(&self) -> &[u8]`, empty by default;
   - `block::serve` answers `model` from it, cut to 40 bytes. Anything not printable ASCII becomes `?`, so that no driver can put a control character into a person's list.
3. **`vfs_server`'s installer** asks each disk's driver for `model` and fills `disk.model`. A driver of 2.0 that refuses the call is not possible before the first release: every driver is built from this tree (AGENTS.md, section 3).
4. **`docs/api`** and `install.wit`'s comment on `model` say where it comes from.

## Acceptance criteria

- [ ] `block.wit` 2.1 generated, and the IDL fuzzer covers `model`.
- [ ] Host test: `serve` answers a driver's model cut to 40 bytes, with a control byte turned to `?`, and an empty one for a driver that keeps none.
- [ ] The `iso` suite's install and the block groups pass on both architectures. The disks' `model` stays empty there until 352-DRV-0118 fills it (QEMU's `virtio_blk` has none).
- [ ] DRV told that 2.1 is in `main`, so that 352-DRV-0118 can start.

## Related

- [352-DRV-0118](352-DRV-0118-the-drives-model-for-block-wit.md) (the drivers' part), [352-KRN-0269](../issues-done/352-KRN-0269-the-installer-service.done) (the installer service).
