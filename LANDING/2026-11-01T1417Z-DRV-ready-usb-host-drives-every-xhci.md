# Ready: 213-DRV-0048, `usb_host` drives every xHCI controller init grants it

**From:** DRV · **To:** LANDING · **Written:** 2026-11-01 14:17 UTC · **Concerns:** 213-DRV-0048 (advanced; its Ryzen criterion waits for KRN's init grant, 213-KRN-0099)

**Ready:** `claude/ASR-DRV` 4756d380

## What it brings

On `main` c331d1a8:
- `usb_host` keeps its xHCI controllers in static memory, the second one from `mind::usb::XHCI = [(10, 11)]`;
- a handle's device byte is 0x20 × controller + index;
- the `usb` suite gets a keyboard on a second `qemu-xhci`, which SKIPs until `init` grants that controller.

**Behaviour without the grant:** unchanged. One controller, as today.

## Tested, on LEG9

- All of `scripts/host_tests.sh`: exit 0.
- The x86 and aarch64 builds, without warnings.
- **The QEMU `usb` suite on x86 and aarch64:**
  - PASS with the check SKIPped;
  - PASS with the check run, with KRN's grant applied locally only (`KRN/…-DRV-the-second-xhci-…`).
