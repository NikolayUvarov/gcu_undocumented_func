# DRV landing request: the Ryzen's display engine surveyed, and `display`'s host-side modules (213-DRV-0085, 213-DRV-0086)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-21 17:17 UTC · **Concerns:** 213-DRV-0085 and 213-DRV-0086 (both advanced, both stay open), 213-DRV-0078

**Ready:** `claude/213-DRV-0085-the-radeons-display` 9a6534754

## What it carries

On `main` 3c5ef74ca (merged in). In `fast-test` 79994efb8, as part of the branch of the request that follows this one.

- **213-DRV-0085, the survey:** `hwdocs/cezanne-display.md` (the display paths from the VBIOS's tables, the state of both outputs under Linux, the monitors' modes, the DMUB question answered from Linux 7.0's source, a route to first light, a register read proposed as safe) and `scripts/hardware/amd_vbios_tables.py`.
  - The issue records the hang of 13:28 UTC on the Ryzen as it happened, and that two of its criteria are not met (the firmware's own state was not read; the machine was not left unchanged).
  - The VBIOS image and the monitors' EDID bytes are not in the repository.
- **213-DRV-0086, host-side only:** `display/src/timing.rs`, `edid.rs`, `dcn.rs` as module files, with no `Cargo.toml`, `main.rs` or linker script, so nothing here meets 213-KRN-0270's crate. `dcn.rs` refuses BAR 5's MSI-X bytes (0x42000 to 0x43FFF) by construction.
- **`THIRD_PARTY.md`:** the facts taken from AMD's MIT-licensed display code and where from; no code of it is copied.
- **No ABI or IDL change.** No image holds the new modules; `hwdocs/` is copied into the images.

## Tested

- **On LEG9:**
  - the tree 7c4a8e554 (before `main` was merged in): `02_build.sh` (x86) passes; the full `scripts/host_tests.sh` passes (238 test binaries, with `MIND_SSHD`), `tests/display_host.rs` among them (15 cases); `tests/usb_image_test.py` passes;
  - the tree 9a6534754 (`main` merged in, no conflict): `tests/display_host.rs` alone, 15 of 15. The build and the full host tests were not run again on it.
- **Not tested:** anything on the Ryzen: none of this code runs there. The survey's route is a plan (MC-12.3).
