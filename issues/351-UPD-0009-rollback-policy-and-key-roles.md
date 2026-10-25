# 351-UPD-0009 — Rollback policy, metadata expiry, key roles and rotation

**Type:** update (policy) · **Owner:** `UPD` track · **Priority:** P2 · **Status:** open (split into 351-UPD-0024 to 0031) · **Blocked by:** its parts, [351-UPD-0024](351-UPD-0024-version-floor-in-the-boot-records.md), [351-UPD-0025](../issues-done/351-UPD-0025-expiry-and-the-offline-policy.done), [351-UPD-0026](351-UPD-0026-root-keys-and-a-signed-key-file.md), [351-UPD-0027](351-UPD-0027-key-rotation-and-revocation.md), [351-UPD-0028](351-UPD-0028-a-short-lived-timestamp-key.md), [351-UPD-0029](351-UPD-0029-the-boot-key-under-the-root.md), [351-UPD-0030](351-UPD-0030-a-separate-recovery-authority.md), [351-UPD-0031](351-UPD-0031-key-lifecycle-and-compromise-protocol.md) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.4, MC-9.5, MC-9.6, MC-11.9

Numbered by the kernel session at the maintainer's request (2026-10-08), before the track had an owner.

## Problem

A signature alone lets an attacker replay an old, signed, vulnerable release (MC-9.4). A single key with no roles cannot be rotated without trusting the network that delivers the new key (MC-9.6).

## Plan

- **The minimum version.**
  - The channel's minimum is kept in the boot records, and the bootloader refuses a slot below it. The highest confirmed version is not a floor: a rollback to an earlier release at or above the minimum stays allowed (the maintainer, 2026-10-25).
  - The profile states the limit plainly: without a hardware counter (TPM NV index, authenticated UEFI variable), someone with the disk can roll back.
  - The hardware floor is [351-UPD-0011](351-UPD-0011-version-floor-in-the-tpm.md), with Secure Boot under our own keys, [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md).
- **Expiry.** Channel metadata past its expiry is refused. The policy for a device that has been offline longer than the expiry is defined: it keeps running, and refuses to update until fresh metadata comes.
- **Roles, after TUF and Appendix B.5:**
  - a root key set that signs the other keys, kept offline, with a threshold;
  - the release key;
  - a short-lived timestamp key for the channel file;
  - a separate recovery authority.

  The root is built into the bootloader and the updater. Rotation is a signed key file.
- **The compromise protocol is written down.** It covers what happens to releases signed by a revoked key, and how recovery still works.

## Acceptance criteria

Tests refuse:

- a replayed older release;
- expired metadata;
- a release signed by a revoked key;
- a key change not signed by the root threshold.

The procedures are in `docs/update.md`.

## Split (2026-10-23)

The maintainer asked for this task to be split. It stays open as their parent, and is done when they are.

What is done already (351-UPD-0007, 351-UPD-0013):

- the channel carries a minimum and an expiry;
- the updater refuses an expired channel, and a clock it cannot read counts as expired;
- it refuses a rollback below the highest minimum it has seen.

| Part | Task | This task's criterion it meets |
|---|---|---|
| A version floor in the boot records, kept by the bootloader | [351-UPD-0024](351-UPD-0024-version-floor-in-the-boot-records.md) | a replayed older release |
| Expiry of metadata and the offline policy, written down and shown | [351-UPD-0025](../issues-done/351-UPD-0025-expiry-and-the-offline-policy.done) | expired metadata |
| Root keys with a threshold, and a signed key file | [351-UPD-0026](351-UPD-0026-root-keys-and-a-signed-key-file.md) | a key change not signed by the root threshold |
| Key rotation and revocation | [351-UPD-0027](351-UPD-0027-key-rotation-and-revocation.md) | a release signed by a revoked key |
| A short-lived timestamp key for the channel | [351-UPD-0028](351-UPD-0028-a-short-lived-timestamp-key.md) | — (freezes, Appendix B.5) |
| The boot key under the root, in the bootloader | [351-UPD-0029](351-UPD-0029-the-boot-key-under-the-root.md) | — (the boot key's rotation) |
| A separate recovery authority | [351-UPD-0030](351-UPD-0030-a-separate-recovery-authority.md) | — (separate recovery authorities, MC-9.4) |
| The key lifecycle and the compromise protocol, written down | [351-UPD-0031](351-UPD-0031-key-lifecycle-and-compromise-protocol.md) | the procedures, now in `docs/update/keys.md` |

## Related

[351](351-self-update.md), [350](350-signed-boot-images.md).
