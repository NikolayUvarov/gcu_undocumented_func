# 351 — Self-update: fetch over HTTPS or SSH, verify, stage in a slot, activate with last-known-good

**Type:** main task · **Owner:** `UPD` track (open) · **Priority:** P1 · **Status:** open · **Blocked by:** [350](350-signed-boot-images.md) (manifest and signature checks) for the signed parts · **Roadmap:** track C (A/B activation with last-known-good, key roles), track D (transports) · **Constitution:** MC-8.5, MC-9.1–9.6, MC-9.9, MC-3.11, MC-11.9; Appendix B.5

Asked by the maintainer (2026-10-08): build the kernel and the tools, publish them on a server, have the system fetch them over SSH and HTTPS, and update itself.

## Problem

Nothing of it exists (survey of 2026-10-08):

- The bootloader loads fixed names from the first file system it finds. It checks only the ELF structure: no hash, signature, slot, counter or fallback.
- `init` starts services from the images the bootloader put in RAM, so a new version takes effect only at a reboot.
- `vfs_server` lets the shell write only to `data/` and `ram:`; no component may replace boot files. FAT has no atomic replace.
- The TLS 1.3 client works with certificate checks, but only the shell may use it, and the shell's `https` shows 2 KiB and saves nothing. There is no HTTP client for programs and no download to disk.
- No SSH code exists. Most of its cryptography does, in the `tls` and `keystore` crates: X25519, Ed25519, ChaCha20-Poly1305, AES-GCM, SHA-2 and HMAC.
- No release format, signing key, server layout or publishing script exists. `keystore`'s device key is made anew at every boot.
- The profile's Article 9 row says "not met — declared".

## Design

- **The channel does not make an update authentic; the signature does (MC-9.2).** HTTPS and SSH are two ways to fetch the same signed files. A file whose signature or hash does not verify is refused, whatever brought it. A mirror or a USB stick can carry the same files.
- **On the server**, static files:
  - `channels/<name>.json`: the latest version, the minimum version, an expiry time and the release manifest's hash, signed by the release key;
  - `releases/<version>/manifest`: the manifest of 350, signed;
  - `blobs/<sha256>`: every file by content.

  Any HTTPS server and any SSH account with a directory serve it. The build host uploads with OpenSSH (`rsync` or `scp`). The upload credential and the release signing key are different keys with different holders (MC-9.6).
- **On the device:**
  - **Slots.** The boot volume holds two system slots, `\MIND\A\` and `\MIND\B\`, each with a kernel, services, programs and its manifest. One stable bootloader, `\EFI\BOOT\BOOTX64.EFI`, chooses the slot.
  - **Boot records.** The choice is in two boot records, `\MIND\BOOT0` and `\MIND\BOOT1`, each with a sequence number, the slot, the tries left and a CRC. The writer overwrites the older one, so a power cut leaves the other valid. This works without an atomic rename.
  - **Trial and confirmation.** The new slot boots on trial with one try. `init` confirms it after `[INIT] READY` and a health check. Without a confirmation before a deadline the system restarts, and the bootloader takes the last-known-good slot (MC-9.3).
  - **The activation point is a reboot.** Updating services while they run (MC-9.9) is out of scope and stated so.
- **Rollback (MC-9.4).** The device refuses a release older than the minimum it has seen, and channel metadata past its expiry. Without hardware that can hold a counter against someone with the disk (TPM, authenticated UEFI variables), this protects against the network, not against physical access. The profile says so.
- **Keys (MC-9.6).**
  - roots, which sign key changes, held offline;
  - the release key;
  - the upload credential;
  - the device's own key for SSH, which must persist, so `keystore` needs sealed storage.

  Rotation and revocation are 351-UPD-0009.

## Plan: tasks by track

**Phase 1 — slots and fallback, tested in QEMU without a network.**

| Task | Track | What |
|---|---|---|
| [350](350-signed-boot-images.md) | `UPD` | Manifest, signing tool, verification in the bootloader, launch record (planned there) |
| [351-UPD-0006](351-UPD-0006-slots-and-boot-records.md) | `UPD`, with `PRT` and `KRN` for the bootloader | Slots A and B, the two boot records, the trial try and the fallback in the bootloader |
| [351-KRN-0014](351-KRN-0014-trial-boot-and-confirmation.md) | `KRN` | The trial flag in `BootInfo`, the confirmation from `init`, a deadline that restarts an unconfirmed trial, the updater's grants |
| [351-ASR-0005](351-ASR-0005-power-loss-during-update.md) | `ASR` (open) | Power cut at every step of staging and activation in QEMU; the system always comes back on a slot that verifies |

**Phase 2 — fetch over HTTPS and publish.**

| Task | Track | What |
|---|---|---|
| [351-UPD-0005](351-UPD-0005-release-and-publish.md) | `UPD` | Release bundle and server layout; `scripts/publish_release.py` signs and uploads over SSH; a test server for CI |
| [351-UPD-0008](351-UPD-0008-update-zone-in-vfs.md) | `UPD` (`vfs_server` has no owner) | An update zone: a badge that may write only the inactive slot and the boot records |
| `requests-NET.md`: HTTPS downloads | `NET` | HTTPS for a service besides the shell; a streaming GET with resume into a file; the update server's trust (root store or pinned key); names and larger volumes in `netpolicy` |
| [351-UPD-0007](351-UPD-0007-updater-service.md) | `UPD` | The `updater` service: check the channel, verify, fetch into the inactive slot, verify every file, write the trial record, ask to restart, report |
| `requests-APP.md`: `update` | `APP` | `update check / fetch / apply / status / rollback` in the shell and `msh`, with a confirmation |

**Phase 3 — SSH.**

| Task | Track | What |
|---|---|---|
| `requests-NET.md`: SSH client | `NET` | An SSH client: curve25519-sha256, ssh-ed25519, chacha20-poly1305; public-key login with the device key; SFTP reads. The updater fetches the same files over it |
| `requests-NET.md`: a persistent device key | `NET` | `keystore` keeps the device key across boots in sealed storage, and gains a purpose for SSH login |

**Phase 4 — rollback policy, keys, the bootloader itself, storage.**

| Task | Track | What |
|---|---|---|
| [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) | `UPD` | Minimum version, expiry, key roles, rotation and revocation, the compromise protocol (MC-9.4, 9.6) |
| [351-UPD-0010](351-UPD-0010-updating-the-bootloader.md) | `UPD` with `PRT` | Updating `BOOTX64.EFI` itself without a single point of failure (two UEFI boot entries, `BootNext`) |
| [351-ASR-0006](351-ASR-0006-update-threat-model.md) | `ASR` (open) | The update threat model (rollback, freeze, mix-and-match, endless data, slow retrieval) and fuzzing of the metadata parser |
| [351-STO-0006](351-STO-0006-releases-pinned-in-the-store.md): staging and pins | `STO` | Once the block store has a durable medium: releases staged as objects, with the last-known-good pinned against collection (MC-9.3) |

Apple Silicon: the same slots work behind U-Boot's UEFI. A first stage started by iBoot ([210-APL-0014](210-APL-0014-own-first-stage-instead-of-m1n1.md)) must not need recoveryOS for an update.

## Acceptance criteria

- A release built on the host is published by `scripts/publish_release.py`. A device running the previous release fetches it over HTTPS, and in phase 3 over SSH, verifies it, and boots it on trial and confirms it. A device that cannot confirm comes back on the previous release by itself.
- A release with a bad signature, a changed file, an older version or expired channel metadata is refused, with a test for each.
- The profile's Article 9 row says what is met, on which platforms, and what is not: physical rollback without a hardware counter, and live service updates.

## Related

[350](350-signed-boot-images.md), [211-KRN-0012](211-KRN-0012-boot-volume-identity.md) (the boot volume the slots live on), Constitution Article 9 and Appendix B.5, ROADMAP track C.
