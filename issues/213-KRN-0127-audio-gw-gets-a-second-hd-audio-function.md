# 213-KRN-0127 — `audio_gw` gets a second HD Audio function, and `init` stops both of a driver's devices before a restart

**Type:** `init` · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (`init`'s part in `main` at 1b1436a; the drivers track's `audio_gw` and the Ryzen's run left) · **Blocked by:** the drivers track's `audio_gw` change (it probes the second function) · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-1.5, MC-6.3, MC-12.1

## Problem

**The request.** The drivers track's request of 2026-10-20 (`internal-dev-messages`, `KRN/2026-10-20T0653Z-DRV-request-audio-gw-gets-every-hda-controller.md`).

The Ryzen has two HD Audio functions:
- `05:00.1 1002:1637`, the GPU's HDMI audio;
- `05:00.6 1022:15E3`, the platform's, with the analog codec.

`init` gives `audio_gw` the first one it finds, the HDMI one, which has no output path. So the machine has no sound.

**Found while doing it.** Before a driver restarts, `init` stops its device's DMA and clears its region (MC-6.3), but only for the first device. `virtio_input`'s second device (the keyboard beside the tablet) kept its DMA running while its region was cleared. A second HD Audio function would have done the same.

## Plan

1. **A second HD Audio function** (class 04:03, the second found) gets its BAR0 in `SLOT_DEV1` and its line in slot 7. The first keeps `SLOT_DEV0`, `SLOT_IRQ` and the one DMA region; `audio_gw` drives one controller at a time.
2. **`init` records a driver's second device**, `audio_gw`'s and `virtio_input`'s. Before a restart it stops both devices' DMA, and after it starts both again.
3. **The drivers track's part:** `audio_gw` probes `SLOT_DEV1` when the first function has no output path, and logs which one it drives.

## Acceptance criteria

- [ ] The QEMU suites pass as before: one HD Audio function, or AC97, and nothing in `SLOT_DEV1` for HD Audio.
- [x] A QEMU case with two `intel-hda` controllers: `stat caps` of `audio_gw` shows the second one's registers in slot 3 and its line in slot 7.
- [ ] On the Ryzen, with the drivers track's change, `audio_gw` drives `05:00.6`, and `beep` and `say` are heard.

**The gate (2026-10-20).** Gate g0131 passed (`c23e754` on Effector's machines; the two groups with the updater suite again on `1b1436a`), and `main` is at 1b1436a. The suites with one HD Audio function or AC97 pass as before; that `SLOT_DEV1` stays empty with one function is not asserted by a test yet.

## Related

[213](213-amd-ryzen-pc.md), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md) (HD Audio in `audio_gw`), `requests-DRV.md` (the Ryzen's sound).
