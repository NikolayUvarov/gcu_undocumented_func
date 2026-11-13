# 351-UPD-0029 — The boot key under the root, in the bootloader

**Type:** update (boot, keys) · **Owner:** `UPD` track, with `KRN`/`PRT` for `bootloader/` · **Priority:** P2 · **Status:** in progress · **Blocked by:** [351-UPD-0026](../issues-done/351-UPD-0026-root-keys-and-a-signed-key-file.done) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.5, MC-9.6

Split from [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) (the maintainer, 2026-10-23).

## Problem

The bootloader checks the manifest with one boot key built into it. Changing that key still needs a new loader at `EFI/BOOT/`: since [351-UPD-0010](../issues-done/351-UPD-0010-updating-the-bootloader.done) a slot carries its own loader, but the one at `EFI/BOOT/` checks it against the slot's manifest with its own built-in key. A compromised boot key cannot be revoked through the key file the updater reads.

## Plan

- **The bootloader carries the root's public keys and threshold**, built in. It reads the key file from the slot, checks it, and checks the manifest with the boot keys the file names. It parses with `libmind` `release`, shared with the updater.
- **The highest key-file version** is kept in the boot records beside the version floor ([351-UPD-0024](../issues-done/351-UPD-0024-version-floor-in-the-boot-records.done)). A slot whose key file is below it is refused.
- **The updater stages the key file** into the slot with the boot files.
- **The transition** (MC-12.4): while no key file has been seen, the bootloader accepts the built-in boot key. Once one has been, it requires one in every slot.
- The bootloader change is made together with `KRN`/`PRT` (TRACKS.md).

## Acceptance criteria

- QEMU, x86 and aarch64:
  - a slot whose manifest is signed by a boot key its key file names boots;
  - after the boot key is rotated, a slot signed by the old key is refused at boot;
  - a key file not signed by the root threshold stops the boot with `BOOT ERROR: KEYS: <reason>`.
- docs/update/README.md and docs/update/slots.md (with slots_RU.md) give the check.

## Progress

**Made (2026-11-13)** on `claude/351-UPD-0029-boot-keys`, on 351-UPD-0030's branch:
- **The bootloader** (`bootloader/build.rs`, `src/keychain.rs`, `src/verify.rs`, `src/main.rs`) is built with the root set (`$MIND_ROOT_KEYS` or `updater/keys/root-test.pub`, the updater's), and no boot key. Before a volume's manifest it checks the volume's `KEYS` under that root, through `KEYS.1`, `KEYS.2`, … of the root's changes: each signed by the threshold of the root before it and of its own, the versions rising. It parses with `libmind/src/release.rs`, the updater's and the parser's. The manifest's signature is then checked against the key file's `boot` role, which must have threshold 1: a manifest carries one signature. A refusal stops the boot with `BOOT ERROR: KEYS: <reason>`; the serial line names the file and the threshold.
- **A version, not the history.** The bootloader holds no key file. It takes a root change only through its file, so the slot carries the files of the root's changes, not every version: `KEYS.1` on, at most 8.
- **The records** (format 3; `bootloader/src/slots.rs`, `scripts/boot_slots.py`) hold the key file version at bytes 32–39, carried like the floor. A slot whose key file is below it is refused. The updater's confirmation of a slot writes that slot's key file version into them, never lower.
- **The transition** follows the maintainer's decision of 2026-10-25 (AGENTS.md, section 3) rather than the plan's: no slot is booted without a key file, and records of format 2 are not read but named, as format 1 before them. The plan was written before that decision.
- **The updater** (`updater/src/main.rs`, `plan.rs`) does the following:
  - It checks each manifest against the trusted key file's `boot` role; no boot key is built in.
  - It keeps each key file it takes that changes the root in `system/updater/roots/<version>`.
  - It stages them as `KEYS.1` on, with `KEYS`, into the slot.
  - It checks a slot it applies, or recovers to, with the bootloader's own check (`keychain.rs`, the parser reading each file).
  - Before it raises the records' version, it gives the other slot the trusted key files, when a boot key they name signed its manifest, so that the other slot stays a fallback.
- **The build and the scripts:**
  - `sign_manifest.py` writes `KEYS` with the manifest: the last of the history in `$MIND_KEYS`, else the test key file, which now has a fixed start (`release.TEST_KEYS_SINCE`) so that every image and test server holds the same bytes. Its command line refuses a real boot key the key file does not name.
  - `make_usb_image.py` puts the key files in the image and slot A.
  - `release.py check` checks manifests against `keys/current`'s `boot` role.
- **Docs:** docs/update/README.md 0.2, slots.md and slots_RU.md 1.2, updater.md and updater_RU.md 0.14, publishing.md and publishing_RU.md 0.8; the profile's Article 9 row and `tcb.md`.
- **Tests:**
  - `tests/boot_keys_host.rs` (new): the check, its root changes and every refusal.
  - `tests/boot_slots_host.rs` and `tests/updater_host.rs`: format 3, and the version carried and never lowered.
  - `tests/release_test.py`, `tests/usb_image_test.py`, `tests/manifest_test.py`, `tests/link_update_test.py`.
  - QEMU, the `boot` suite: no key file, and one key file short of the root threshold.
  - QEMU, the `updatebootkey` suite (new, in the updater group): a boot key rotated through key file 2. Release 7, signed by the new key, is staged, booted on trial and confirmed, and the records hold version 2. Slot A is then refused at boot, by its key file's version and by its old key's signature, and slot B boots.
- **Tested in this container:**
  - the host and Python tests above pass, `tests/manifest_test.py`'s key file check among them (9eb5465f);
  - the bootloader builds for both targets and the updater too, with no warning;
  - QEMU x86: the `boot` suite (the key file refusals; records of formats 1 and 2 named and not read) and the `updatebootkey` suite pass.
- **Run** at this branch with `main` ada2b041 merged in, at 629398b3, on the maintainer's machines (Effector, QEMU under TCG):
  - x32-128: "x86: the updater and an update through the link" (1089 s), "x86: boot, display, shell, memory, clock" (731 s), "x86: USB image" (106 s), "x86: Secure Boot with our keys" (23 s), "x86: reproducible build" (232 s), "x86: NVMe boot disk" (83 s);
  - its second lane: "host tests" (977 s), "x86: files and block writes" (933 s), "x86: keys, shell, tools" (633 s), "x86: a developer's release through the Effector agent" (48 s), "x86: network, TLS and Effector" (1415 s);
  - acc-2: "aarch64: the updater and the modes" (1046 s), "aarch64: boot and fault containment" (174 s), "aarch64: an update through the link" (743 s), "aarch64: the board from a device tree, under U-Boot" (117 s), "aarch64: network, TLS and Effector" (1234 s), "aarch64: the link" (211 s), "aarch64: GICv2 with GICv2m" (851 s);
  - PCU: "aarch64: programs, shell and four CPUs" (588 s), "aarch64: NVMe boot disk" (234 s), "aarch64: files and the block store" (1110 s).
  Every one passed.

## Related

[351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0024](../issues-done/351-UPD-0024-version-floor-in-the-boot-records.done), [351-UPD-0026](../issues-done/351-UPD-0026-root-keys-and-a-signed-key-file.done), [351-UPD-0027](../issues-done/351-UPD-0027-key-rotation-and-revocation.done), [351-UPD-0010](../issues-done/351-UPD-0010-updating-the-bootloader.done), [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md).
