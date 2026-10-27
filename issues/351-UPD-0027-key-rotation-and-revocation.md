# 351-UPD-0027 — Key rotation and revocation

**Type:** update (keys) · **Owner:** `UPD` track · **Priority:** P2 · **Status:** open · **Blocked by:** [351-UPD-0026](351-UPD-0026-root-keys-and-a-signed-key-file.md) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.5, MC-9.6

Split from [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) (the maintainer, 2026-10-23).

## Problem

With a key file ([351-UPD-0026](351-UPD-0026-root-keys-and-a-signed-key-file.md)), a key is replaced by publishing a new file. Two gaps remain:

- nothing stops an old key file from being served again, which brings a revoked key back;
- nothing says what happens to releases signed by a key the file no longer lists.

## Plan

- **The highest key-file version is kept.** The updater keeps the highest version it has accepted on the disk, in the update zone beside the boot records, and refuses a lower one.
- **The root rotates like TUF's.** A key file that changes the root is signed by the threshold of the old root and of the new one.
- **A key the file lists no more is revoked.** A channel signed only by it is refused, at a check and at a rollback: a slot whose kept channel carries only that key's signature is not a fallback. The boot key at boot is [351-UPD-0029](351-UPD-0029-the-boot-key-under-the-root.md).
- **Commands:** `scripts/release.py keys --rotate ROLE` and `--revoke KEY`.

## Acceptance criteria

- Host tests:
  - a lower key-file version is refused;
  - a root change signed by one of the two thresholds is refused.
- QEMU `update` suite, x86 and aarch64:
  - after the release key is rotated, a channel signed by the old key is refused (a release signed by a revoked key);
  - an older key file served again is refused;
  - the kept version survives a restart of the updater and a reboot.
- docs/update/publishing.md and publishing_RU.md give the procedure.

## Related

[351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0026](351-UPD-0026-root-keys-and-a-signed-key-file.md), [351-UPD-0029](351-UPD-0029-the-boot-key-under-the-root.md), [351-UPD-0031](351-UPD-0031-key-lifecycle-and-compromise-protocol.md).
