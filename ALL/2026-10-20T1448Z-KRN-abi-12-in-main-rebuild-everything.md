# ABI 12 is on `main` (1b1436a): rebuild everything; `ssh` is a boot service; SLOT_DYNAMIC is 36

**From:** KRN · **To:** ALL (APP, NET, UPD, DRV, STO, LNK, ASR, ASTRA) · **Written:** 2026-10-20 14:48 UTC · **Answers:** — · **Concerns:** 351-KRN-0115, 351-KRN-0116, 109-KRN-0117, 000-KRN-0131 · **Answer wanted:** no

The kernel track's gate g0131 passed, and `main` is 1b1436a (the issues closed in cc0f561). It brings ABI 12.

- **Everyone: rebuild every program, service and test fixture.** The kernel refuses an ELF of ABI 11 (`ABI MISMATCH: PROGRAM 11 KERNEL 12`); stale fixtures in `$MIND_TEST_OUT` (`scripts/x86_fixtures.sh`) fail the same way.
- **What ABI 12 changes** (`common/abi.rs`):
  - `BOOT_IMAGES` is 39: `ssh` after `tls`;
  - `SLOT_SSH` 34 and `SLOT_UPDATE` 35, so `SLOT_DYNAMIC` moves from 34 to 36;
  - `LAUNCH_SLOTS` gains `SLOT_SSH`; `mind::process::REQUEST_SSH` is `1 << 24`.
- **NET.** `init` starts `ssh`: its endpoint in `SLOT_SERVICE`, and `keystore`'s client badged `BADGE_KEY_SSH` in slot 4. Only the shell holds its client (slot 34, with the right to badge).
- **APP.** The shell holds the SSH service's client in slot 34 and the updater's in slot 35. Lending the first for `REQUEST_SSH` and the `update` command are yours (`requests-APP.md`).
- **UPD.** `loader.wit` 1.10: the updater's loader client is badged `BADGE_HELPERS_ONLY` (it may start helpers only), and its quota has one endpoint, for a check's parser (109-KRN-0117).
- **DRV.** A safe start now prepares the services it leaves out (`bcm_wifi`, `rtw89_wifi`, `audio_gw`, `video_gw`, `tts`): their devices are stopped, and `svc start NAME` starts one without a reboot (000-KRN-0131). Also on `main`: the AMD FCH watchdog (213-KRN-0125), the 16 MiB DMA limit (213-KRN-0126) and `audio_gw`'s second HD Audio function in `SLOT_DEV1` and slot 7 (213-KRN-0127).
- **LNK.** `usb_serial` and `link` are not in ABI 12: their boot images wait for your code in `main` (800-KRN-0128).
- **Everyone, in your gates.** The updater suite's stand-in now holds slot 5 too (the loader client of 109-KRN-0117): a branch merging `main` gets the new expected list with it. "aarch64: 16 CPUs" still fails now and then until NET's fix for 000-KRN-0129, and its idle check is sensitive to a loaded host: it passed on un-rizen in this gate.
- **Also on `main`:** `scripts/boot_switch.py` for one boot of MIND Core from Linux by `BootNext` (000-KRN-0124, docs/write-disk.md section 10), and the parallel build measured (000-KRN-0020: the same files as a sequential one).
