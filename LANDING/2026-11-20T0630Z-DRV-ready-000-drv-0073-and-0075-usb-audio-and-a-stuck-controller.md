# DRV landing request: USB Audio Class 1 and isochronous streams (000-DRV-0073, with 213-KRN-0263), and an xHCI that stops answering set up again (000-DRV-0075)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-20 06:30 UTC · **Concerns:** 000-DRV-0073, 000-DRV-0075, 213-KRN-0263, 801 S5

**Ready:** `claude/000-DRV-0073-usb-audio` 448c3472e

This replaces `2026-11-20T0150Z-DRV-ready-000-drv-0073-usb-audio-and-xhci-streams.md`, which had no answer yet. Everything listed there is still carried, now on `main` 934483c3b (merged in, ABI 35). The branch is in `fast-test` acdbf6de6.

## What it adds since that request

- **000-DRV-0075, `usb_host`:**
  - a command waits 5 s, as Linux's does, not 30 s;
  - one with no answer marks the controller stuck, and the main loop sets it up again;
  - a controller that does not halt is reset all the same;
  - the device it stopped at is left alone, by root port and route, until it is unplugged;
  - 8 restarts in a row, each within 10 minutes of the one before, leave the controller off.
- **Why.** On the Ryzen, the STM-Speaker plugged straight into a root port of the first AMD xHCI (05:00.3) stopped that controller at its set-up, in MIND Core and in Linux alike. The boot stick and the Bluetooth on it were lost until a restart, and each 30 s wait also stopped the other controller's keyboards and link.
- **000-DRV-0073's issue:** progress notes only. It stays open.

## Tested, on LEG9 (the tree 448c3472e)

- `build (x86)`, `host tests`, `build (x86 test programs)`, `x86: keys, shell, tools` (usb, ehci), `x86: the window manager` and `x86: files and block writes` pass under `ci_local.sh`.
- The suites `services`, `audio`, `tts`, `listen`, `hda`, `usbaudio`, `link` and `modes` pass one by one.
- `ahci` still fails here as it does on `main` (`run files &` gives no PID after the driver's restart), so that group's other suites were run one by one. The machines' gate decides `ahci` and aarch64.

## Tested, on the Ryzen

Release 202611200415 is the ABI 34 tree with 0075 cherry-picked. It went in through `data/link/rel`, and its trial was confirmed 6 s after ready. It enumerates the Bluetooth and the boot stick on the first controller, and the hub, keyboard and link on the second. The stuck-controller path itself waits for the speakerphone to be plugged in under it.
