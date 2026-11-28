# 352-KRN-0294 — `block.wit` 2.1: the drive's model as it names itself, for the installer's disk list

**Type:** IDL (`idl/block.wit`), `libmind` (`mind::block::serve`), `vfs_server` (the installer) · **Owner:** `KRN` (the kernel session) · **Priority:** P3 · **Status:** in progress (2026-11-28) · **Blocked by:** — · **Main task:** [352](352-installable-media-for-pcs-and-arm-devices.md) · **Constitution:** MC-12.3, MC-12.4, MC-12.7

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

- [x] `block.wit` 2.1 generated, and the IDL fuzzer covers `model`.
- [x] Host test: what `serve` answers (`shown_model` of the driver's `model`) is cut to 40 bytes, with a control byte turned to `?`, and empty for a driver that keeps none.
- [ ] The `iso` suite's install and the block groups pass on both architectures. The disks' `model` stays empty there until 352-DRV-0118 fills it (QEMU's `virtio_blk` has none).
- [ ] DRV told that 2.1 is in `main`, so that 352-DRV-0118 can start.

## Progress

**2026-11-28, on `claude/352-KRN-0294-drive-model`:**
- **`idl/block.wit` 2.1.0:** `model: func() -> string<40>`, the last call, so 2.0's calls keep their numbers. `libmind/src/idl/block.rs` was generated again, and `scripts/mind_idl.py --check` passes.
- **`mind::block`:**
  - `Driver::model`, empty by default;
  - `block_protocol::shown_model`, which cuts trailing spaces and NULs, keeps at most `MODEL_MAX` (40) bytes and turns any byte outside printable ASCII into `?`;
  - `serve` answers `model` with it, and with nothing without a drive;
  - `Device::model` asks the driver, and gives an empty model when it does not answer.
- **`vfs_server`:** `Io`, `Disk` and `Shared` give the drive's model. The installer asks each disk once, when it lists the disks, and keeps it after an install, as the drive is the same.
- **`install.wit`** and `docs/idl/README.md` say where the model comes from.
- **Tests:** `tests/block_host.rs` passes 10 tests, the new one among them. `tests/idl_fuzz_host.rs` passes. `vfs_server` builds for x86 and aarch64 with no warning.
- **Left:** the `iso` suite's install and the block groups on both architectures (on the machines), and DRV told once 2.1 is in `main`.

## Related

- [352-DRV-0118](352-DRV-0118-the-drives-model-for-block-wit.md) (the drivers' part), [352-KRN-0269](../issues-done/352-KRN-0269-the-installer-service.done) (the installer service).
