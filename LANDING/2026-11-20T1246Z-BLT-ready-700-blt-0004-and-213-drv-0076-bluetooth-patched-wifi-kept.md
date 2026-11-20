# BLT/DRV landing request: the RTL8852B patched (700-BLT-0004), and the coexistence's high-LNA bit as Linux writes it (213-DRV-0076)

**From:** BLT and DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-20 12:46 UTC · **Concerns:** 700-BLT-0004, 213-DRV-0076, 000-DRV-0075 (closed), 213-DRV-0052

**Ready:** `claude/700-BLT-0004-rtl8852b-patch` b2bf26f9e

This replaces `2026-11-20T1123Z-BLT-ready-700-blt-0004-the-rtl8852b-patched.md`, which had no answer yet. **Please do not land 700-BLT-0004 without 213-DRV-0076:** once the controller is patched, `rtw89_wifi`'s old high-LNA rule makes the Bluetooth scoreboard flip hundreds of times a second. Both are on this branch.

## What it carries

On `main` 7144476c3 (merged in); in `fast-test` 4a67709d3.
- **700-BLT-0004, `bt_hci`:**
  - patches the RTL8852B from `rtl_bt/rtl8852bu_fw.bin` ("RTBTCore" format), fetched by `scripts/firmware/rtl8852b_bt.py` into the ignored `proprietary/`;
  - download fragment indices past 255 fixed (the 256th was 0x80, the last one's mark).
- **213-DRV-0076, `rtw89_wifi` (`btc_context.rs`):**
  - the WLAN scoreboard's high-LNA bit is written only while Bluetooth's own bit differs from what is wanted, as Linux's `_set_bt_rx_agc`;
  - the old rule cleared ours whenever Bluetooth's was set, and the two chased each other;
  - a host test in `tests/rtw89_connection_host.rs`.
- **Text:**
  - 000-DRV-0075 closed with its Ryzen evidence;
  - 000-DRV-0073's progress (the controller's set-up compared with Linux's);
  - the README's `bt_hci` line;
  - hwdocs/rtl8852b-bt.md and THIRD_PARTY.md.
- **No ABI or IDL change.** The listening still waits for `data/bt-listen.txt`.

## Tested

- **On LEG9** (before the merge of main 7144476c3, which brings 801-UPD-0045 only): `build (x86)`, `host tests` (all of `host_tests.sh`), `build (x86 test programs)` and `x86: boot, display, shell, memory, clock` pass.
- **After the merge:** `02_build.sh`, `tests/bt_hci_host.rs` (8 passed, with the real file) and `tests/rtw89_connection_host.rs` (260 passed) again. aarch64 is left to the gate.
- **On the Ryzen,** test builds of both services on the stick's ABI 34 tree, through `replace`, with an inquiry every 80 s:
  - **without 0076:** 48 inquiries in about 70 minutes, three data-mode stops, scoreboard flips hundreds a second;
  - **with it:** 24 inquiries in 33 minutes, no stop, one scoreboard change;
  - **the patch itself:** `RTL8852B PATCHED: LMP SUBVERSION 4E04, HCI REVISION 42D3`; LE scans hear 8–11 devices and inquiries 3–4, where the ROM heard none.
- **Not tested here:** the DEXP's RTL8821C on hardware.
