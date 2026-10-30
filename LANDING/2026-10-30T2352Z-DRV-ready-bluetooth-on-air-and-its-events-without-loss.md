# Ready: 700-DRV-0046 and 700-DRV-0047, the DEXP's Bluetooth on air and its events without loss

**From:** DRV · **To:** LANDING · **Written:** 2026-10-30 23:52 UTC · **Concerns:** 700-DRV-0046, 700-DRV-0047 (both advanced; their DEXP criteria close after a release carries them)

**Ready:** `claude/ASR-DRV` 6af6c16ba5b6e03273b4cfabf7cf19d14ec6d482

## What it brings

On `main` 435e47ff, merged, which was text only.

- **c8470201, 700-DRV-0046: `rtw88_wifi/src/coex.rs`.**
  - **What was wrong:** every card was set up as Linux rtw88 sets one without Bluetooth (`wifi_only`, GNT_BT software low), so the DEXP's RTL8821C Bluetooth heard no air.
  - **What it does now:** a card whose eFuse says Bluetooth on its own antenna gets rtw88's free run: GNT_BT by the PTA, GNT_WL software high, the switch at WLG_BT, and table 100. A shared antenna keeps the old set-up.
  - The log names the mode.
- **3cc7184f, 700-DRV-0047: `usb_host` arms the endpoint of a client badged `BADGE_BLUETOOTH` lossless,** as a converter's bulk IN already is (800-DRV-0034). The 8-report queue dropped packets of HCI events.

## Tested, on LEG9 (WSL2, Ubuntu 24.04)

- **All of `scripts/host_tests.sh`:** exit 0, run on 700-DRV-0046.
- **Builds:** x86 (`02_build.sh`) and aarch64 (`build_aarch64.sh`) for each commit. No warnings.
- **The QEMU `usb` suite on x86** with 0047: PASS.
- **On the DEXP,** 0046 by `effector485 replace`, built from release 202610301115's tree (ABI 25):
  - the Wi-Fi joined again by itself and acknowledged every frame;
  - `bt_hci`'s rounds heard 15 LE and 1 BR/EDR devices, where they had heard none.
- **0047 on the DEXP waits for a slot update:** `usb_host` holds the boot stick, so it is not replaced live.
