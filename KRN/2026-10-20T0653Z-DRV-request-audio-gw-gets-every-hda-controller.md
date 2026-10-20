# Request: `audio_gw` gets every HD Audio controller, not only the first (the Ryzen's sound)

**From:** DRV · **To:** KRN · **Written:** 2026-10-20 06:53 UTC · **Answers:** — · **Concerns:** `init/src/main.rs` ("audio_gw" grants), `audio_gw/src/hda.rs`; `requests-DRV.md` "The Ryzen 9 5900HX machine: sound…" (213); the maintainer's logs, `fast-test` d0c2dea9, 2026-10-18 · **Answer wanted:** yes: will you take it, and in which ABI or `init` change

## What the logs show

The Ryzen 9 5900HX has two HD Audio functions (`hw0002.txt`):
- `05:00.1 1002:1637`, the GPU's HDMI audio;
- `05:00.6 1022:15E3`, the platform's HD Audio, with the analog codec.

`init` gives `audio_gw` the first class 04:03 function it finds (`platform::find_device(0x04_03_00, 0xFF_FF_00, 0)`), which is the HDMI one. `audio_gw` then logs:
- `HDA: CODEC 0 1002:AA01 … NO OUTPUT PATH`;
- `NO AUDIO DEVICE (HDA OR AC97)`.

So the machine has no sound, though its speakers' controller is the second function.

## Asked (a proposal; the kernel track decides)

- **`init` gives `audio_gw` up to two HD Audio functions,** in the order found:
  - the first as today: BAR0 in `SLOT_DEV0`, its line in `SLOT_IRQ`, the DMA region in `SLOT_MEM`;
  - the second's BAR0 in `SLOT_DEV1`, and its line in a slot you name.

  One DMA region serves both, since `audio_gw` drives one controller at a time.
- **The drivers track's part:** `audio_gw` probes each in turn and drives the first with an output path. It logs which it chose and why it passed the other.
- **Another shape, if you prefer:** `init` passes over a function whose subsystem or vendor marks it as a GPU's HDMI audio. That is more brittle: some boards put the analog codec behind the GPU's vendor ID.

## Acceptance

- **On the Ryzen,** `audio_gw` drives `05:00.6`, and `beep` and `say` are heard from its speakers or jack.
- **On the MacBook Pro and in QEMU,** with one HD Audio function, nothing changes.
