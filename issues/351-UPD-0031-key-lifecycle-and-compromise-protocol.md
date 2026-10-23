# 351-UPD-0031 — The key lifecycle and the compromise protocol, written down

**Type:** update (documentation, policy) · **Owner:** `UPD` track · **Priority:** P2 · **Status:** open · **Blocked by:** [351-UPD-0026](351-UPD-0026-root-keys-and-a-signed-key-file.md), [351-UPD-0027](351-UPD-0027-key-rotation-and-revocation.md), [351-UPD-0028](351-UPD-0028-a-short-lived-timestamp-key.md), [351-UPD-0029](351-UPD-0029-the-boot-key-under-the-root.md), [351-UPD-0030](351-UPD-0030-a-separate-recovery-authority.md) (for the procedures they show; the document may start before) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.6, MC-11.9

Split from [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) (the maintainer, 2026-10-23).

## Problem

MC-9.6 asks for each key's lifecycle: creation, assignment, storage, use, rotation, revocation and destruction. It also asks for a compromise protocol:

- what happens to the images, authorities and data signed before;
- whether recovery stays available;
- what autonomous nodes do.

MC-11.9 keeps access to key material apart from the right to ask for a signature. None of this is written down: docs/update/publishing.md says "not defined yet".

## Plan

- **docs/update/keys.md and keys_RU.md**, for each role (root, release, timestamp, boot, recovery, the test keys, and the Secure Boot keys of [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md) by reference):
  - where it is made and who holds it;
  - where it is stored (offline, the publishing server, a hardware token);
  - what it signs;
  - how it is rotated, revoked and destroyed.
- **The compromise protocol**, for each role:
  - which releases stay valid (those signed before and listed again in a new channel) and which are refused;
  - how devices learn of it (the key file);
  - what an offline device does ([351-UPD-0025](351-UPD-0025-expiry-and-the-offline-policy.md));
  - how recovery stays available ([351-UPD-0030](351-UPD-0030-a-separate-recovery-authority.md));
  - what a key change does not undo (MC-9.6, last sentence).
- **MC-11.9.** Signing runs through `scripts/release.py`, with a seed read from a file or a token and never from the command line. A device holds no key that signs updates.
- **Each procedure names its command** and the test that shows it.
- docs/update/publishing.md's "not defined yet" lines and the profile's update entries are brought up to date.

## Acceptance criteria

- keys.md exists in English and Russian.
- Each step names its command and its test from [351-UPD-0024](351-UPD-0024-version-floor-in-the-boot-records.md) to [351-UPD-0030](351-UPD-0030-a-separate-recovery-authority.md).
- The profile lines are updated.
- The document has been checked against MC-9.4 to 9.6 and MC-11.9, clause by clause.

## Related

[351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0026](351-UPD-0026-root-keys-and-a-signed-key-file.md), [351-UPD-0027](351-UPD-0027-key-rotation-and-revocation.md), [351-UPD-0028](351-UPD-0028-a-short-lived-timestamp-key.md), [351-UPD-0029](351-UPD-0029-the-boot-key-under-the-root.md), [351-UPD-0030](351-UPD-0030-a-separate-recovery-authority.md), [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md), [SECURITY.md](../SECURITY.md).
