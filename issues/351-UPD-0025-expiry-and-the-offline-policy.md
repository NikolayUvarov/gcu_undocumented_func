# 351-UPD-0025 — Expiry of metadata and the offline policy, written down and shown

**Type:** update (policy) · **Owner:** `UPD` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.4

Split from [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) (the maintainer, 2026-10-23).

## Problem

What is done: the updater refuses an expired channel, and it counts a clock it cannot read as expired (docs/update/updater.md). The `update` suite shows the refusal over HTTPS.

What is not done:

- the policy for a device that has been offline longer than the expiry is not written down;
- the directory source (`linkupdate`) has not shown the refusal;
- `status` does not say that the metadata the device holds has expired.

MC-9.4 asks that expiry during autonomous operation follow a predefined policy, and Appendix B.5 that freezes be checked.

## Plan

- **The policy**, in docs/update/updater.md and updater_RU.md. A device past its metadata's expiry:
  - keeps running its confirmed slot;
  - refuses to fetch or apply until fresh metadata comes;
  - still allows a rollback to a version not below the floor ([351-UPD-0024](351-UPD-0024-version-floor-in-the-boot-records.md)), since that needs no network;
  - says so in `status` and in the log.

  A clock that cannot be read counts as expired.
- **`status`** gives the held channel's expiry and whether it has passed: a new field in `idl/update.wit`, as a minor version.
- **The `linkupdate` suite** refuses an expired channel from a directory.
- **Freezes.** A server that keeps serving the same unexpired channel can hold a device on it until the channel expires. A shorter bound is the timestamp key ([351-UPD-0028](351-UPD-0028-a-short-lived-timestamp-key.md)).

## Acceptance criteria

- The `update` and `linkupdate` suites refuse an expired channel, on x86 and aarch64, with no record written.
- `status` shows the expiry and that it has passed.
- The policy is in docs/update/updater.md and updater_RU.md.

## Related

[351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done), [351-KRN-0141](351-KRN-0141-an-update-through-the-link.md), [351-UPD-0028](351-UPD-0028-a-short-lived-timestamp-key.md).
