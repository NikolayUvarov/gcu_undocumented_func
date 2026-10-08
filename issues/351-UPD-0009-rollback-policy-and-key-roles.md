# 351-UPD-0009 — Rollback policy, metadata expiry, key roles and rotation

**Type:** update (policy) · **Owner:** `UPD` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** [351-UPD-0007](351-UPD-0007-updater-service.md) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.4, MC-9.5, MC-9.6, MC-11.9

Numbered by the kernel session at the maintainer's request (2026-10-08); the track is open.

## Problem

A signature alone lets an attacker replay an old, signed, vulnerable release (MC-9.4). A single key with no roles cannot be rotated without trusting the network that delivers the new key (MC-9.6).

## Plan

- **The minimum version.**
  - The highest confirmed version and the channel's minimum are kept in the boot records, and the bootloader refuses a slot below them.
  - The profile states the limit plainly: without a hardware counter (TPM NV index, authenticated UEFI variable), someone with the disk can roll back.
  - A hardware counter is a later issue.
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

## Related

[351](351-self-update.md), [350](350-signed-boot-images.md).
