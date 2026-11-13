# 351-UPD-0030 — A separate recovery authority

**Type:** update (keys, policy) · **Owner:** `UPD` track · **Priority:** P2 · **Status:** in progress · **Blocked by:** — (made on 351-UPD-0028's branch, which carries 351-UPD-0027) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.4, MC-9.6

Split from [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) (the maintainer, 2026-10-23).

## Problem

MC-9.4 asks a permitted rollback to take separate recovery authorities into account. MC-9.6 and Appendix B.5 ask that recovery power be kept apart from normal installation, and that it not bypass accounting.

Today the only way below the floor is writing the disk by hand. And once the release key is revoked, a fix can only ship under the new one, so recovery has to keep working across that change.

## Plan

- **A recovery role** in the key file, with its own keys and threshold, kept apart from the release and timestamp keys.
- **A recovery order** is signed by the recovery threshold. It names:
  - the channel it applies to;
  - the version to go back to;
  - the lower floor;
  - its own sequence number and expiry.

  The updater checks it, lowers the floor, writes a trial record of that slot, and logs the order.
- **Accounting** (Appendix B.5): every order applied is logged and kept with the records. An order whose sequence number is not above the last one is refused, so it cannot be replayed.
- **After a revocation**, an order can point to a release signed under the new release key.

## Acceptance criteria

- QEMU `update` suite, x86 and aarch64:
  - a rollback below the floor signed by the release key is refused;
  - the same, signed by the recovery threshold, is applied and logged;
  - an order replayed is refused.
- docs/update/updater.md and updater_RU.md give the procedure.

## Progress

**Made (2026-11-13)** on `claude/351-UPD-0030-recovery`, on 351-UPD-0028's branch:
- **The recovery role** is the key file's `recovery` (351-UPD-0026), apart from `release` and `timestamp`.
- **The order** (`libmind/src/release.rs` `Order`, `parse_order`; `idl/parse.wit` 4.2 `order`): `{"channel":…,"expires":…,"floor":…,"sequence":…,"version":…}`, 1 ≤ floor ≤ version, and the recovery keys' signature lines. Served as `recovery/<channel>` (`tools/httpd`, `mind::http_serve`).
- **The updater** reads it at each check, after the key file. It applies it when the recovery threshold signed it, it is the channel's, its sequence is above the last applied, it has not expired, the running slot is confirmed and the other slot holds its version as the bootloader will check it. Applying it, the updater first adds a line to `system/updater/recovery` (the account), then writes the other slot's trial record with the floor lowered (`plan::recover`, the one write that lowers it), logs the order and asks init to restart. A refusal is logged once.
- **Accounting:** the account in `system/updater`, read at start (`RECOVERY ORDER n … APPLIED BEFORE`), and each order applied in the system log. The plan said "kept with the records"; the records are written only in place, 512 bytes each, so the account sits in the updater's private directory instead, as the key file does (351-UPD-0027).
- **The publisher:** `scripts/release.py recover DEST --version V --floor F [--days 7]`, signed by `$MIND_RECOVERY_KEY` (the test recovery key without it), its sequence one above the server's.
- **Docs:** `docs/update/updater.md` and `updater_RU.md` 0.13, `publishing.md` and `publishing_RU.md` 0.7, the profile's Article 9 row, `docs/idl/README.md`.
- **Tested** (this container): `tests/updater_host.rs` (`plan::recover`), `tests/release_host.rs` (the order's form), `tests/release_test.py` (`recover`), `tests/idl_fuzz_host.rs`, `tests/http_serve_host.rs`; `updater` builds for x86.
- **The QEMU check** `update-recovery` (in the `updater` suite), x86 and aarch64: release 6 in slot A under floor 6, release 5 in slot B. An order back to 5 the release key signed is refused with no record written. The recovery key's is applied: slot B boots on trial under floor 5 and is confirmed. The same order, still served, is refused as replayed. Runs on the maintainer's machines: below, once made.

## Related

[351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0024](../issues-done/351-UPD-0024-version-floor-in-the-boot-records.done), [351-UPD-0026](../issues-done/351-UPD-0026-root-keys-and-a-signed-key-file.done), [351-UPD-0031](351-UPD-0031-key-lifecycle-and-compromise-protocol.md), [305](../issues-done/305-recovery-without-the-store.done).
