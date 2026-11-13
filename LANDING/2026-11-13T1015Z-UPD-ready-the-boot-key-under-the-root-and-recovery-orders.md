# UPD ready: the boot key under the root, and recovery orders

**Ready:** `claude/351-UPD-0029-boot-keys` 32d0486c48

**Closes** 351-UPD-0029 and 351-UPD-0030 once it lands; 351-UPD-0031's document waits on it (text, `claude/351-UPD-0031-keys`).

**What it changes:**
- **The bootloader** (KRN/PRT field, `bootloader/`) is built with the root set and no boot key.
  - It checks a volume's `KEYS` under that root, through `KEYS.1` on of the root's changes (`src/keychain.rs`, host-tested), then the manifest against the key file's boot role.
  - A refusal stops with `BOOT ERROR: KEYS: <reason>`.
- **Boot records, format 3:** they hold the key file version, and a slot below it is not booted. Records of format 2 are named and not read, and a slot without `KEYS` does not boot; a disk of before this is written again (the maintainer's rule of 2026-10-25).
- **The updater** checks manifests against the trusted key file's boot role. It stages the key files with a slot, and its confirmation raises the records' version.
- **The scripts:** `sign_manifest.py` writes `KEYS`, the test key file now with a fixed start, so images stay reproducible.
- **351-UPD-0030:** recovery orders (`recovery/<channel>`, `parse.wit` 4.2, `release.py recover`).

**Tested** at 629398b3, the branch with `main` ada2b041 merged in, on the maintainer's machines (QEMU under TCG). Every group passed:
- x32-128:
  - updater and link (1089 s);
  - boot, display, shell (731 s);
  - USB image (106 s);
  - Secure Boot (23 s);
  - reproducible build (232 s);
  - NVMe (83 s).
- Its second lane:
  - host tests (977 s);
  - files and block writes (933 s);
  - keys, shell, tools (633 s);
  - a developer's release (48 s);
  - network, TLS and Effector (1415 s).
- acc-2, aarch64:
  - updater and modes (1046 s);
  - boot and fault containment (174 s);
  - an update through the link (743 s);
  - U-Boot board (117 s);
  - network, TLS and Effector (1234 s);
  - the link (211 s);
  - GICv2 (851 s).
- PCU, aarch64:
  - programs, shell and four CPUs (588 s);
  - NVMe (234 s);
  - files and the block store (1110 s).

In this container, the `boot` and `updatebootkey` QEMU suites passed. The last two commits add `tests/manifest_test.py`'s key file check (run here) and the issues' records.
