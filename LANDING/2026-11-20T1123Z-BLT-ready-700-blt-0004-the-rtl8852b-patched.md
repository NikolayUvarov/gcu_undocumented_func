# BLT landing request: bt_hci patches the Ryzen's RTL8852B (700-BLT-0004)

**From:** BLT · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-20 11:23 UTC · **Concerns:** 700-BLT-0004, 213-DRV-0052, 801

**Ready:** `claude/700-BLT-0004-rtl8852b-patch` 7f82d4e3d

## What it carries

On `main` c03e0184b; in `fast-test` e12c8393f.

- **`bt_hci`:**
  - reads Realtek's "RTBTCore" patch format, from its layout ([hwdocs/rtl8852b-bt.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/700-BLT-0004-rtl8852b-patch/hwdocs/rtl8852b-bt.md));
  - patches the RTL8852B (LMP 0x8852, HCI revision 0x000B) from `data/firmware/rtl_bt/rtl8852bu_fw.bin`, checked by size and SHA-256;
  - **a fix that also concerns the RTL8821C:** download fragment indices after 0x7F run 1..0x7F again; the 256th fragment used to get 0x80, the last one's mark. The RTL8821C's 217 fragments never reached it.
- **`scripts/firmware/rtl8852b_bt.py`**, run by `scripts/proprietary.sh fetch`: the file from the pinned linux-firmware revision. It goes into the ignored `proprietary/` only; THIRD_PARTY.md names the script and the source.
- **Docs:** README's `bt_hci` line, the issue and its README row.
- **No ABI or IDL change.** The listening still waits for `data/bt-listen.txt` (700-BLT-0003).

## Tested

- **On LEG9:** `build (x86)`, `host tests` (all of `host_tests.sh`, `tests/bt_hci_host.rs` 8 passed with the real file), `build (x86 test programs)` and `x86: boot, display, shell, memory, clock` pass. aarch64 is left to the gate.
- **On the Ryzen,** a test build of this `bt_hci` on the stick's ABI 34 tree, through `replace`:
  - `RTL8852B PATCHED: LMP SUBVERSION 4E04, HCI REVISION 42D3`;
  - LE scans hear 8–11 devices and inquiries 3–4, where the ROM heard none;
  - the Wi-Fi firmware passes BT_INFO now;
  - 11 inquiries in 23 minutes, 8 of them 80 s apart, without the Wi-Fi losing its network.
- **Not tested here:** the DEXP's RTL8821C on hardware. Its image and fragments are host-tested.
