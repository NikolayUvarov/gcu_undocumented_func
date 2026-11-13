# 351-UPD-0027 — Key rotation and revocation

**Type:** update (keys) · **Owner:** `UPD` track · **Priority:** P2 · **Status:** in progress · **Blocked by:** — ([351-UPD-0026](../issues-done/351-UPD-0026-root-keys-and-a-signed-key-file.done), landed in round 127) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.5, MC-9.6

Split from [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) (the maintainer, 2026-10-23).

## Problem

With a key file ([351-UPD-0026](../issues-done/351-UPD-0026-root-keys-and-a-signed-key-file.done)), a key is replaced by publishing a new file. Two gaps remain:

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

## Progress

**Made (2026-11-13)** on `claude/351-UPD-0027-key-rotation`, on 351-UPD-0026's branch:
- **The root role.** A key file names a fifth role, `root`: the keys and threshold that sign the next version (`libmind/src/release.rs` `ROLES`; `idl/parse.wit` 4.0, its `key-file` of five roles; 3.x's of four is gone, by the rule of 2026-10-25).
- **The step** (`release::step`): from the version held, a key file is `Next` (version + 1, signed by the held root's threshold and its own), `Same`, `Lower`, `Skips` or `Unsigned`. Version 1's root before it is the one built into the updater.
- **The device** (`updater`):
  - it holds one key file, kept in `system/updater/keys`, a private directory only `BADGE_UPDATE` opens (`vfs_server` `PRIVATE`); written whole beside it as `keys.new`, then put in its place, and read back from either at start;
  - it takes `keys/current` only as the next version, each version between read from `keys/<version>` first; a lower one is refused (`older`), the same one taken only as the same file;
  - without a kept file it takes a slot's `KEYS` when that is the next version;
  - a channel, a slot's kept one included, is checked against the release keys of the held key file: a slot whose channel only a revoked key signed is no fallback, and its version is not known; a release written again from the block store is checked the same way.
- **Beside the plan:** the highest version is kept in `system/updater`, not beside the boot records: those are written only in place, 512 bytes each (`vfs_server`'s `Zone::Record`), and a key file is up to 4096 bytes and replaced whole.
- **The publisher** (`scripts/release.py`):
  - the server keeps every version as `keys/<version>`, `keys/current` the last;
  - `keys publish` publishes only the next version, checked against the last one's root and its own;
  - `check` walks the history from `$MIND_ROOT_KEYS`'s set;
  - `bundle` holds the history;
  - `keys rotate FILE ROLE OUT --key … --threshold T` and `keys revoke FILE KEY OUT` write the next version's body (the plan's `--rotate ROLE` and `--revoke KEY`, as subcommands like the others).
- **The layout:** `tools/httpd` and `mind::http_serve` serve `keys/<version>` as a file that never changes; `scripts/link_update.py` puts the history before the key file.
- **Docs:** `docs/update/publishing.md` and `publishing_RU.md` 0.5, with the procedure; `updater.md` and `updater_RU.md` 0.11; `docs/profile/README.md`'s Article 9 row; `docs/profile/bootstrap.md`'s `updater` row; `docs/idl/README.md`.
- **Tested** (this container):
  - `tests/release_host.rs`: a lower key-file version is refused; a root change signed by one of the two thresholds is refused, by either one;
  - `tests/release_test.py`: rotation and revocation by the tool, a version published out of turn, a history with a version missing;
  - `tests/updater_host.rs`, `tests/vfs_zone_host.rs`, `tests/http_serve_host.rs`, `tests/link_update_test.py`, `tests/idl_fuzz_host.rs`, `tools/httpd`'s tests;
  - `updater`, `parse`, `vfs_server` and `init` build for x86, `updater` for aarch64.
- **The QEMU check** `update-keys` (`tests/qemu_smoke.py`, in the `updater` suite), x86 and aarch64: versions 2 (the release key rotated) and 3 (the root rotated) taken in turn and the channel the old release key signed refused; version 1 served again refused, and a version 4 the rotated-away root signed; the channel the new key signed taken; after `svc restart updater` and after a reboot, version 3 read back and version 1 refused; slot A, whose channel only the revoked key signed, no fallback.
- **Run** in this container (x86, QEMU 8.2.2 under TCG) at b2736f71: `--suites updatekeys` passed, every step above; `--suites update` passed, 351-UPD-0026's key-file case now as version 2 refused by version 1's root.
- Runs on the maintainer's machines: below, once made.

## Related

[351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0026](../issues-done/351-UPD-0026-root-keys-and-a-signed-key-file.done), [351-UPD-0029](351-UPD-0029-the-boot-key-under-the-root.md), [351-UPD-0031](351-UPD-0031-key-lifecycle-and-compromise-protocol.md).
