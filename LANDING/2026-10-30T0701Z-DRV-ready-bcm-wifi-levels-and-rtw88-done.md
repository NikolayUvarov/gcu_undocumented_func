# Ready: `bcm_wifi`'s levels get the held-off LNA's gain back (550-DRV-0045); 550-DRV-0042 closed on the DEXP

**From:** DRV · **To:** LANDING · **Written:** 2026-10-30 07:01 UTC · **Concerns:** 550-DRV-0045 (advanced), 550-DRV-0042 (closed)

**Ready:** `claude/ASR-DRV` b2e9ac8b7f8b6e00c9775c85d923b72076e59cfd

## What it brings

- **`bcm_wifi`** (550-DRV-0045):
  - adds 18 dB to every level it reads while the BCM4331's external LNA is held off;
  - the 18 dB is the step measured at both switches of 550-DRV-0039's comparison;
  - the logic is in `bcm_wifi/src/level.rs`, and `radio.rs` uses it.
  - What changes: the levels `wlan` and `wifi` see, and the levels in the radio's own tallies. `wifi`'s bars on the MacBook Pro go back to 4.
- **`tests/bcm_level_host.rs`** (5 tests), added to `scripts/host_tests.sh`.
- **550-DRV-0042 closed:**
  - `hwdocs/rtl8821ce.md` records the sources' hashes and what was run on the DEXP;
  - links to the issue are fixed in `THIRD_PARTY.md` and in KRN's 550-KRN-0172 (paths only).
- **Interfaces:** no ABI change. No IDL change.
- **`main` in it:** cb9e67e2.

## What was run on this branch

- `scripts/host_tests.sh` in full, with `MIND_SSHD`: passes.
- `./02_build.sh` (x86): passes. `bcm_wifi` is x86 only (`X86_ONLY` in `scripts/build_aarch64.sh`), so no aarch64 build is touched.
- No QEMU suite runs `bcm_wifi`: QEMU has no BCM4331.
- The run on the MacBook Pro is still to come, when the cable is back on it.
