# 351-UPD-0005 — The release bundle, the server layout and publishing over SSH

**Type:** update (host tools) · **Owner:** `UPD` track · **Priority:** P1 · **Status:** open · **Blocked by:** 350-UPD-0002 (manifest and signing tool, planned in [350](350-signed-boot-images.md)) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.2, MC-9.6, MC-9.7

Numbered by the kernel session at the maintainer's request (2026-10-08), before the track had an owner.

## Problem

Nothing turns a build into something a device can fetch and check.

## Plan

- **`scripts/release.py`** turns `usb_root/` (x86) and `aarch64_root/` into a release:
  - every file under `blobs/<sha256>`;
  - the manifest of 350 for each architecture;
  - a version: monotonic, never reused.
- **`scripts/publish_release.py`:**
  - signs the channel file `channels/<name>.json` (latest version, minimum version, expiry, manifest hash) with the release key, read from outside the repository;
  - uploads with the system's OpenSSH (`rsync -e ssh`), blobs first and the channel file last, so a reader never sees a channel that names missing files;
  - takes the host, user and directory from the command line or a config file outside the repository. The upload credential is never the release key.
- **A local test server for CI and `scripts/ci_local.sh`:** a directory served over HTTPS with the test CA the TLS suite already makes, and over SSH by OpenSSH on the host if present. The test release key is labelled as such.
- **`docs/update.md`** (English and Russian) describes the layout, how to set up a server, and key custody.

## Acceptance criteria

`publish_release.py` against a local directory yields a layout whose signatures and hashes a host-side checker verifies. Publishing twice the same version is refused. The order of uploads leaves no channel naming a missing blob.

## Related

[351](351-self-update.md), [350](350-signed-boot-images.md).
