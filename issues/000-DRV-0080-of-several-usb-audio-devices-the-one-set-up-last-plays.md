# 000-DRV-0080 — `audio_gw`: of several USB audio devices, the one set up last plays

**Type:** driver (`audio_gw`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-20) · **Blocked by:** — · **Roadmap:** A · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

`audio_gw` took the first USB audio function `usb_host` had, and looked for no other while that one was attached (000-DRV-0073).

**The DEXP has USB audio of its own:** 0573:1573 on its xHCI's port 6 (its HD Audio has no codec). It is set up at every boot and never goes. So a speakerphone plugged in beside it was never used. 000-DRV-0073's "a USB device plugged in later becomes available without a restart" did not hold on such a machine.

## Plan

- **A look for newer devices.** With no stream running, every 2 s the gateway claims the audio interfaces that are not its own yet. Those belong to devices `usb_host` set up since the last look.
- **The one set up last is used.** Each usable device found takes the place of the one in use, the earliest set up first. This is what a headset plugged into a machine with sound of its own is expected to do.
- **The device replaced waits,** its interfaces kept (two devices at most). When the one in use goes, the latest of those waiting that still answers comes back. With none, the codec comes back as before.
- **A running stream is never moved.** The change happens only between streams.

**Not done here (MC-12.3):**
- **At boot** the order is `usb_host`'s set-up order (root ports, then hub ports), not the owner's choice. On the DEXP a speakerphone in a root port before port 6 is set up first, and 0573:1573 plays.
- **A choice the owner makes and the machine keeps** belongs to the audio interface (`idl/audio.wit`), in a new version of it.

## Acceptance criteria

- [x] QEMU (`--suites usbaudio`): a second `usb-audio` plugged in beside the first is attached and the first waits; `beep` then reaches the second one's wav (0.86 s of tone); unplugged, the first is in use again; with the first unplugged too, the codec comes back.
- [ ] On the DEXP: the speakerphone plugged in plays in place of 0573:1573, and with it unplugged 0573:1573 plays again.
  - **Not yet run.** The DEXP has this change since release 202611201947 (2026-11-20), and its own 0573:1573 is attached at each boot as before. The speakerphone is not set up in its root port (000-DRV-0079), so no second device has come. The run needs it through a hub, which needs hands.

## Related

[000-DRV-0073](000-DRV-0073-usb-audio-class-speakerphone.md) (USB audio in `audio_gw`).
