# 213-DRV-0060 — The Ryzen 9 5900HX's second NVMe disk, and its analog sound heard

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** for the second disk, `init`'s grant of a second `nvme` instance (KRN, to be asked when this starts) · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1

Numbered from the kernel track's request of 2026-10-13 in `requests-DRV.md` ("The Ryzen 9 5900HX machine: sound, Ethernet, a second NVMe disk, a second xHCI"), from the maintainer's first run (`log:hw0003.txt`).

## Problem

What the request named, and where each part is now:
- **The second xHCI** (05:00.4): done in [213-DRV-0048](../issues-done/213-DRV-0048-usb-host-drives-every-xhci.done).
- **Ethernet,** the RTL8168 at 02:00.0: [213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md). It carried the machine's traffic and the Effector agent in November.
- **Sound.** `audio_gw` took the GPU's HDMI audio (05:00.1, 1002:1637), found no output path and gave up.
  - It now tries a second HD Audio function when the first has none: `init`'s part is [213-KRN-0127](213-KRN-0127-audio-gw-gets-a-second-hd-audio-function.md), and `audio_gw`'s `Device::open`.
  - The analog controller (05:00.6, 1022:15E3) has not been heard on this machine yet.
- **The second NVMe disk** (04:00.0, 1F40:1202): `init` lends `nvme` only the first controller (01:00.0), and `nvme` drives one.

## Plan

1. **The sound:** on the Ryzen, with a build of `main`:
   - the log shows `TRYING THE SECOND` and `DRIVING THE SECOND FUNCTION`;
   - `beep` plays through the analog output, to the maintainer's ears.

   If the codec has no path `audio_gw` knows, its dump is read and the path added.
2. **The second disk:**
   - `nvme` takes its controller from `init` by instance, as `virtio_blk#1` does;
   - `init`'s part, `nvme#1` with the second controller's BAR0, is asked of KRN;
   - `vfs_server` and the block store see it as another device.

## Acceptance criteria

On the Ryzen:
- a tone plays through its analog output;
- the second NVMe disk is listed with its capacity;
- its first sectors read the same as Linux reads them.

The machine is the maintainer's: the run needs it booted into MIND Core, and the tone needs ears.

## Progress (2026-11-13): the analog output heard; the second disk still not listed

- **Run:** the Ryzen on release 202611131315 (`main` 20fba2c5), speakers on its analog jack.
- **The output:** `audio_gw` drives HDA's second function: Realtek `10EC:0269`, out pin 0x15 (headphone jack) from DAC 0x2.
  - `beep` (440 Hz and 660 Hz) and `say`, run over the link, were heard by the maintainer at 100 % (0 dB of 88 settings).
  - The first criterion is met.
- **The disks:** only one `nvme` instance runs, `NAMESPACE 1: 250069680 SECTORS`. That is the Patriot disk with Windows' partitions.
  - Linux lists a second one, a Netac 128 GB with Ubuntu, and MIND Core lists nothing of it.
  - The second and third criteria are left.

## Related

[213](213-amd-ryzen-pc.md), [213-KRN-0127](213-KRN-0127-audio-gw-gets-a-second-hd-audio-function.md), [213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md), [213-DRV-0048](../issues-done/213-DRV-0048-usb-host-drives-every-xhci.done), [551](551-sound-on-pcs.md).
