# 213-KRN-0263 — `audio_gw` gets a `usb_host` client badged for audio

**Type:** `init` (the service grants) · **Owner:** `KRN`; the line is carried in DRV's commit by agreement (below) · **Priority:** P2 · **Status:** open (2026-11-19) · **Blocked by:** `mind::usb::BADGE_AUDIO`, on DRV's `claude/DRV-ryzen-peripherals` (000-DRV-0073) · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-3.1, MC-3.3

## Problem

Asked by DRV (`KRN/2026-11-19T2212Z-DRV-request-audio-gw-gets-a-usb-host-client-badged-audio.md` on `internal-dev-messages`). The maintainer's USB speakerphone (STM-Speaker, `CAFE:4014`, UAC1) is to play and record on the Ryzen. DRV's part is under way in 000-DRV-0073:
- isochronous streams on xHCI in `usb_host`;
- `usb.wit` 1.3 with an isochronous OUT;
- a UAC1 path in `audio_gw`;
- `mind::usb::BADGE_AUDIO = 6`, which claims interface class 1.

`init` gives `audio_gw` no client of `usb_host`, so the gateway cannot reach a USB audio function.

## Plan

In `init`'s `"audio_gw"` grants, beside the HD Audio and AC'97 ones: when `usb_host` runs, a client of it badged `mind::usb::BADGE_AUDIO`, in slot 8.
- The slot is named `SLOT_AUDIO_USB` in `init/src/main.rs`, beside `SLOT_AUDIO_IRQ1` (7).
- It is given in the same way as `video_gw`'s `SLOT_DEV1`.
- The line is the same with no sound card, so a machine with only a USB device still gets it.

```rust
if self.running(service_index("usb_host")) { grants.add(SLOT_AUDIO_USB, self.badged(&mut minted, "usb_host", mind::usb::BADGE_AUDIO)?, CLIENT); }
```

**Carried by DRV.** `BADGE_AUDIO` exists only on DRV's branch, so this line cannot build on `main` before it. I agreed that DRV carries it in their commit, citing this task (AGENTS.md, section 2: joining an owned track by agreement with its owner). I check it when it lands, as the integrator.

## Acceptance criteria

- [ ] `audio_gw` holds, in slot 8, a client of `usb_host` badged for audio when `usb_host` runs, and nothing there otherwise. QEMU checks this with a USB audio device, in DRV's suite.
- [ ] The grant appears in `docs/profile`'s list of what `init` lends, if that list names `audio_gw`'s.

## Related

[213](213-amd-ryzen-pc.md), 000-DRV-0073 (DRV's), [213-KRN-0127](213-KRN-0127-audio-gw-gets-a-second-hd-audio-function.md) (the second HD Audio function and slot 7).
