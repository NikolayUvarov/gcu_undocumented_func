# 214-DRV-0063 — The Intel Core i5 PC: sound through its analog codec, and its Ethernet

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [214](214-intel-core-i5-pc.md) · **Constitution:** MC-6.1, MC-12.1

Numbered from the kernel track's request of 2026-10-13 in `requests-DRV.md`, from the maintainer's two boots of that PC (`log:boot0001.log`, `boot0002.log`, `hw0001.txt`, `hw0002.txt`).

## Problem

**Sound.** There are three HD Audio controllers:
- **00:03.0** (8086:0C0C), the processor's display audio. `audio_gw` took it, and its codec did not answer ("CORBRP 1 RIRBWP 0"): the screen is on a GeForce GT 1030, so the processor's display is likely off.
- **00:1B.0** (8086:8C20), the analog one, was not tried.
- **01:00.1** (10DE:0FB8), the GT 1030's HDMI audio.

**Ethernet:** a Realtek RTL8168 (10EC:8168, revision 0C, at 03:00.0).

**Since then:**
- `audio_gw` passes over a function whose codec does not answer, or which has no output path, and tries a second one (213-KRN-0127). Here the second is 00:1B.0.
- `rtl8168` exists ([213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md)), and takes the chips whose XID it lists (`XIDS`: 0x4C0 and 0x541).

Neither has run on this PC.

## Plan

1. **A run of `main` on the PC:**
   - the log names which HD Audio function `audio_gw` drives;
   - `beep` plays through the analog output;
   - `rtl8168` names the card's XID.
2. **If the XID is not listed:**
   - its initialization is checked against the pinned sources (`hwdocs/rtl8168.md`);
   - it is added only when the sequence for it is known.
3. **The GT 1030's HDMI audio,** a third function, is not needed: `init` lends two.

## Acceptance criteria

On this PC:
- a tone plays through its analog output;
- its Ethernet card gets an address by DHCP.

The run needs the maintainer at the PC.

## Related

[214](214-intel-core-i5-pc.md), [213-DRV-0060](213-DRV-0060-the-ryzens-second-nvme-and-its-analog-sound.md), [213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md), [213-KRN-0127](213-KRN-0127-audio-gw-gets-a-second-hd-audio-function.md).
