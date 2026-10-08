# 350-UPD-0004 — The launch record: which manifest booted, readable in the system

**Type:** boot (update) · **Owner:** `UPD` track · **Priority:** P2 · **Status:** in progress · **Blocked by:** [requests-KRN.md](requests-KRN.md) ("The launch record, readable in the system") · **Main task:** [350](350-signed-boot-images.md) · **Roadmap:** track C "launch records" · **Constitution:** MC-9.1, MC-9.5

## Problem

After 350-UPD-0003 the bootloader knows which manifest it verified, with which key, and which images it checked. MC-9.1 asks the profile to describe the provenance of executable code, and MC-9.5 includes provenance in the trust evidence. A running system cannot tell which manifest it booted from.

## Plan

- **The bootloader** prints the record on the serial line: the manifest's SHA-256, the key's identity, whether it is the test key, and the images checked. Done with 350-UPD-0003.
- **The kernel** keeps the record from `BootInfo` and makes it readable (a `STAT` class, or a line `init` publishes). This changes the ABI, so it is kernel work, requested in `requests-KRN.md`.
- **A tool** shows it, and the `boot` suite compares it with the serial line.

The record is evidence: nothing grants or refuses on it.

## Acceptance criteria

- A program reads the record of the volume it booted from, and it matches the serial line's, on x86 and aarch64.

## Progress (2026-10-08)

- **Done:** the serial line. `BOOT: MANIFEST <16 hex> KEY <hex16> [(THE TEST KEY)] VERIFIED, <n> IMAGES CHECKED` is printed on every verified boot, x86 (30 images) and aarch64 (27).
- **Waiting:** the `BootInfo` field and the `STAT` class from `KRN`.

## Related

[350-UPD-0003](../issues-done/350-UPD-0003-verification-at-boot.done), [docs/update](../docs/update/README.md).
