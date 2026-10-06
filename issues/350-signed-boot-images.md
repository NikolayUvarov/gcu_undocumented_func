# 350 — Signed boot images and a launch record (track C, first step)

**Type:** main task · **Owner:** `UPD` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track C "Signed manifests and launch records; reproducible toolchain" · **Constitution:** MC-9.1, MC-9.2, MC-9.5, MC-9.7, MC-3.11

## Problem

Article 9 is "not met — declared": the bootloader loads whatever is on the boot volume, and nothing records which images ran. Track C begins with signatures over the boot images and a manifest that lists them. A manifest only describes an image and the authorities it asks for; it never grants them (MC-3.11). The track also needs evidence that the build is reproducible.

## Plan

The tasks below are planned; the `UPD` track numbers them itself.

- `350-UPD-0001` — a reproducible build check: two builds of the same commit in different directories give byte-identical images. The script records what still differs and why (MC-9.7).
- `350-UPD-0002` — a manifest format, versioned. It lists each boot image's name, hash and requested authorities, plus the build inputs (commit, toolchain from `rust-toolchain.toml`, `Cargo.lock` hashes). A tool `scripts/sign_manifest.py` signs it with Ed25519 using a key outside the repository; the test key used in CI is labelled as such.
- `350-UPD-0003` — verification at boot. The bootloader checks the manifest's signature against a public key built into it, and each image's hash against the manifest, before it loads anything. On a mismatch it stops with a message. This changes `bootloader/`, so it is done with `KRN` and `PRT` (`issues/requests-KRN.md`).
- `350-UPD-0004` — a launch record. The kernel or `init` makes the manifest's hash and the verification result readable: a `STAT` class, or a log line `init` publishes. The record is evidence, not authority.
- Later main tasks: A/B activation with last-known-good (MC-9.3, 9.4) and key roles and rotation (MC-9.6).

## Acceptance criteria

- The reproducible build check runs in CI or in `scripts/ci_local.sh`.
- A signed image boots. A changed image or a changed manifest is refused, with a test for each.
- The profile's Article 9 row says what is now met, on which platforms, and states the trust model: where the public key lives and what Secure Boot is or is not involved.

## Related

ROADMAP track C; `bootloader/`, `02_build.sh`, `scripts/make_usb_image.py`; Constitution Article 9.
