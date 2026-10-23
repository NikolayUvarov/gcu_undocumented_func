# 351-UPD-0030 — A separate recovery authority

**Type:** update (keys, policy) · **Owner:** `UPD` track · **Priority:** P2 · **Status:** open · **Blocked by:** [351-UPD-0024](351-UPD-0024-version-floor-in-the-boot-records.md), [351-UPD-0026](351-UPD-0026-root-keys-and-a-signed-key-file.md) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.4, MC-9.6

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

## Related

[351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0024](351-UPD-0024-version-floor-in-the-boot-records.md), [351-UPD-0026](351-UPD-0026-root-keys-and-a-signed-key-file.md), [351-UPD-0031](351-UPD-0031-key-lifecycle-and-compromise-protocol.md), [305](../issues-done/305-recovery-without-the-store.done).
