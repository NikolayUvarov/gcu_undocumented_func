# 350 — Signed boot images and a launch record (track C, first step)

**Type:** main task · **Owner:** `UPD` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track C "Signed manifests and launch records; reproducible toolchain" · **Constitution:** MC-9.1, MC-9.2, MC-9.5, MC-9.7, MC-3.11

## Problem

Article 9 is "not met — declared": the bootloader loads whatever is on the boot volume, and nothing records which images ran. Track C begins with signatures over the boot images and a manifest that lists them. A manifest only describes an image and the authorities it asks for; it never grants them (MC-3.11). The track also needs evidence that the build is reproducible.

## Plan

The tasks below are planned; the `UPD` track numbers them itself.

- `350-UPD-0001` — a reproducible build check: two builds of the same commit in different directories give byte-identical images. The script records what still differs and why (MC-9.7).
- `350-UPD-0002` — a manifest format, versioned. It lists each boot image's name, hash and requested authorities, plus the build inputs (commit, toolchain from `rust-toolchain.toml`, `Cargo.lock` hashes). A tool `scripts/sign_manifest.py` signs it with Ed25519 using a key outside the repository; the test key used in CI is labelled as such.
- `350-UPD-0003` — verification at boot. The bootloader checks the manifest's signature against a public key built into it, and each image's hash against the manifest, before it loads anything. On a mismatch it stops with a message. This changes `bootloader/`, so it is done with `KRN` and `PRT` (`issues/requests-KRN.md`).
- `350-UPD-0004` — a launch record. The kernel or `init` makes the manifest's hash and the verification result readable: a `STAT` class, or a log line `init` publishes. The record is evidence, not authority.
- Later main tasks: A/B activation with last-known-good (MC-9.3, 9.4) and key roles and rotation (MC-9.6).

## Progress (2026-10-08)

Taken by the storage session at the maintainer's request (2026-10-08).

- **Done:**
  - `350-UPD-0001` ([done](../issues-done/350-UPD-0001-reproducible-build-check.done)): `scripts/reproducible.sh`, in `scripts/ci_local.sh`;
  - `350-UPD-0002` ([done](../issues-done/350-UPD-0002-manifest-and-signing.done)): the manifest and `scripts/sign_manifest.py`;
  - `350-UPD-0003` ([done](../issues-done/350-UPD-0003-verification-at-boot.done)): verification in the bootloader, the refusals in the `boot` suite;
  - the profile's Article 9 rows (x86 and aarch64) state the trust model.
- **Open:** [`350-UPD-0004`](350-UPD-0004-launch-record.md). The serial record is done; the record readable in the system waits for the kernel track.

The acceptance criteria below are met; the main task stays open until 350-UPD-0004 is done.

## Acceptance criteria

- The reproducible build check runs in CI or in `scripts/ci_local.sh`.
- A signed image boots. A changed image or a changed manifest is refused, with a test for each.
- The profile's Article 9 row says what is now met, on which platforms, and states the trust model: where the public key lives and what Secure Boot is or is not involved.

## Related

ROADMAP track C; `bootloader/`, `02_build.sh`, `scripts/make_usb_image.py`; Constitution Article 9.
