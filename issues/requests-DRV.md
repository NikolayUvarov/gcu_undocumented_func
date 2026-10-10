# Requests for the drivers track (DRV), not numbered yet

**Owner:** drivers track (the assessing session, `claude/ASR-DRV`, TRACKS 1.5) · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-09

The request on transfers to a gone device became [211-DRV-0021](211-DRV-0021-a-gone-device-fails-at-once.md) (2026-10-09).

The drivers track numbers its own tasks (`NNN-DRV-MMMM`), so requests from other tracks wait here. The drivers track turns each into a task and removes it from this file, and the file goes when it is empty.

## The MacBook Pro's speakers: more of 551's step 2 if the GPIO is not enough (551)

**Recorded by:** the kernel track (KRN), 2026-10-09, for [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md). That task stays with the kernel session until done (TRACKS 1.5).

The kernel session has set the Cirrus codec's amplifier GPIOs for Apple's machines (551-DRV-0010's progress) because the maintainer's run found `beep` and `say` silent. Jack detection, the internal microphone and any further codec setup are 551's step 2, which this track numbers when it takes it.


## `bcm_wifi` stage 1 on the MacBook Pro: the chip is read, the SPROM is not (550-DRV-0020)

**Recorded by:** the kernel track (KRN), 2026-10-10, from the maintainer's run of `fast-test` a00618b (`log:boot0001.log`). [550-KRN-0059](../issues-done/550-KRN-0059-bcm-wifi-at-boot.done) started the driver there.

### What the log shows

- **Start.** `init` started `bcm_wifi` (PID 18) with BAR0.
- **PCI.** `PCI 14E4:4331, BAR0 WINDOWS 18001000 AND 18101000`: window 1 shows core 1 and window 2 its wrapper, as the firmware left them.
- **Chip.** `CHIP 4331 REVISION 2 PACKAGE 9 CORES 3 BUS AI`.
- **ChipCommon.**
  - Capabilities `58500000`: bit 30 says a SPROM is fitted.
  - Chip control `00010092`, status `00000002`, SROM control `00000003`.
  - The enumeration ROM at `18107000`.
- **Core 1, the 802.11 core.** IO control `00002055` (clock on), IO status `0000100C`, not in reset.
- **The PCIe core window** (BAR0 + 0x2000) reads `FFFFFFFF FFFFFFFF FFFFFFFF 00000000`.
- **The SPROM** at ChipCommon + 0x800 and + 0x830 reads all zeros, so it is not valid and the MAC address is not known yet.

### What the kernel track reads in it (to check against the drivers track's sources)

- **Chip control `00010092` has bits 4 and 7 set.** As the kernel track recalls the 4331's chip-control bits from Broadcom's published definitions (the names the b43/bcma documentation uses), these are:
  - bit 4: the external PA enabled;
  - bit 7: the external PA on GPIO 2/5 and the SPROM's data-out pin.

  On package 9 the PA lines share the SPROM's pins. Drivers clear these bits for the SPROM read and restore them after, which is a write to ChipCommon. Stage 1 deliberately writes nothing.
- **The PCIe core window's `FFFFFFFF`** suggests that BAR0 + 0x2000 is not the PCIe core on this revision. That line can go, or be read where the enumeration ROM says the core is.

### Plan (a proposal; the drivers track decides)

- **A next step that writes one thing.** Clear bits 4 and 7 (and the second PA enable, if set) of chip control around the SPROM read, then restore the value read before.
- **Log it.** Log the value before and after, and the MAC address.
- **Compare.** The maintainer can compare the MAC address with the Wi-Fi address macOS shows.

### Acceptance criteria

- On the MacBook Pro the SPROM reads valid (CRC good).
- Its MAC address matches the one macOS shows for Wi-Fi.
- Chip control reads back as it was before.

## A volume in `audio.wit`, for `wm`'s Settings (000-APP-0048)

**Recorded by:** the tools track (APP), 2026-10-10, for [000-APP-0048](000-APP-0048-wm-settings.md) (the maintainer's request: one place for settings, the sound among them).

### Problem

`audio.wit` plays, records and makes tones, but has no volume: a program can only scale its own samples. Settings has nothing to set for the sound.

### Plan (a proposal; the drivers track decides)

- `audio.wit` 1.x: `volume: func() -> u8` and `set-volume: func(percent: u8)`, on the codec's mixer where it has one (AC'97 master volume, the HDA output amplifier) and by scaling in `audio_gw` otherwise; setting it only with a badge the shell holds.
- The shell sets it for Settings (the tools track's part).

### Acceptance criteria

The `audio` suite sets the volume to 50 % and the recorded tone's level falls by about 6 dB; a client without the badge is refused.
