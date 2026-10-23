# 351-UPD-0026 — Root keys with a threshold, and a signed key file

**Type:** update (keys) · **Owner:** `UPD` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.5, MC-9.6, MC-12.4

Split from [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) (the maintainer, 2026-10-23).

## Problem

The updater and the bootloader each carry one key built in: the release key and the boot key (docs/update/README.md). Changing either needs a new build, which is trusted through the very key it replaces. Nothing above them can say which keys are current (MC-9.5, 9.6).

## Plan

- **A key file**, after TUF's root metadata and Appendix B.5:
  - its version and its expiry;
  - for each role (release, timestamp, boot, recovery), the Ed25519 public keys and the threshold;
  - signed by root keys: one `ed25519 <key id> <signature>` line each, after the body.

  It has one encoding, as the channel has. `libmind` `release` reads it and encodes it again, so the updater checks the parser's answer against the signed bytes (as 351-UPD-0013 does for the channel). Roles added later, such as the application-package key of [750-UPD-0019](750-UPD-0019-signed-application-packages.md), take their place in the same file.
- **A root key set** of n keys with a threshold t (for example 2 of 3):
  - its public keys are built into the updater and the bootloader, as the release key is now (a build input, `$MIND_ROOT_KEYS`);
  - each root seed is kept offline, apart from the others;
  - test root keys are derived from fixed text, as the test key is, and the log names them as test keys.
- **`scripts/release.py keys`** makes a key file, signs it with the root seeds given one at a time, and checks one.
- **The updater:**
  - fetches the key file with the channel and checks it against the root threshold;
  - checks the channel with the release keys it names, not the built-in one.

  The built-in release key holds only until the first key file is published. From the release that ships with one, the updater requires it: an explicit transition (MC-12.4).
- **A fuzz target** for the new parser is asked of `ASR` (351-ASR-0006's harness).

## Acceptance criteria

- Host tests:
  - the key file is read and encoded again to the same bytes;
  - a file with fewer than t valid root signatures is refused;
  - two signatures by one root key count once;
  - a signature by an unknown key is ignored.
- QEMU `update` suite, x86 and aarch64:
  - a channel signed by a release key the key file names is taken;
  - a key file signed by one root key of a 2-of-3 threshold is refused, with no record written (a key change not signed by the root threshold).
- docs/update/publishing.md and publishing_RU.md give the format and the command.

## Related

[351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0027](351-UPD-0027-key-rotation-and-revocation.md), [351-UPD-0028](351-UPD-0028-a-short-lived-timestamp-key.md), [351-UPD-0029](351-UPD-0029-the-boot-key-under-the-root.md), [351-UPD-0030](351-UPD-0030-a-separate-recovery-authority.md), [351-UPD-0013](../issues-done/351-UPD-0013-release-metadata-in-libmind.done), [750-UPD-0019](750-UPD-0019-signed-application-packages.md).
