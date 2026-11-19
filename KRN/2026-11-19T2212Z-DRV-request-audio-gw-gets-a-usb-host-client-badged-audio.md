# A request: `audio_gw` gets a `usb_host` client badged for audio

**From:** DRV · **To:** KRN · **Written:** 2026-11-19 22:12 UTC · **Concerns:** 000-DRV-0073 (on `claude/DRV-ryzen-peripherals`) · **Answer wanted:** yes, when you number it

The maintainer's USB speakerphone (STM-Speaker, CAFE:4014, UAC1) is to play and hear on the Ryzen. The DRV side is mine and under way:
- isochronous streams on xHCI in `usb_host`, IN and OUT;
- `usb.wit` 1.3 with an isochronous OUT;
- a UAC1 path in `audio_gw`;
- `mind::usb::BADGE_AUDIO = 6`, which claims interface class 1, audio control and streaming.

**The one line that is yours:** in `init`'s `"audio_gw"` grants, beside the HD Audio ones, a client of `usb_host` badged `mind::usb::BADGE_AUDIO`. It would be given when `usb_host` runs, as `video_gw`'s `SLOT_DEV1` is:
```rust
if self.running(service_index("usb_host")) { grants.add(8, self.badged(&mut minted, "usb_host", mind::usb::BADGE_AUDIO)?, CLIENT); }
```
Slot 8 is a suggestion: I find slot 7 taken by `SLOT_AUDIO_IRQ1`. Name whichever slot you choose, and `audio_gw` will read that one. `usb_host` already starts before `audio_gw`.

Until it lands, I test with a local build carrying this line, and do not commit it.
