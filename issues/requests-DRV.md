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

## `usb_storage` reports a device that came back, rather than running the command on it (000-KRN-0079, 000-KRN-0085)

**Recorded by:** the kernel track (KRN), 2026-10-11.

### Problem

Since 211-DRV-0019, a command that finds the device gone claims it again. If a device of the same capacity answers, the command runs on it (`THE DEVICE IS BACK, THE SAME CAPACITY`).

Our images all have the same capacity, so that device may be a stick rewritten with another build. `vfs_server` checks the medium before it writes when it saw the drive not answer (000-KRN-0079). But it can make no request while the stick is out, for example when everything it needs is cached. Then the first request after the swap is run on the new stick, possibly a write of the old system's cached sectors, before anything can be checked.

### Plan (a proposal; the drivers track decides)

- When the device comes back, the command that found it is not run. It fails, and `usb_storage` reports the return to the client, so the client checks the medium first.
- Libmind's block protocol carries the report (000-KRN-0085, the kernel track's part):
  - `Driver::returned()` is read by `block::serve` before each request;
  - a request after a return is answered "not found" and the client's buffer is dropped;
  - a client that asks to check the medium learns of the return instead of having the request repeated.

### Acceptance criteria

In QEMU (`tests/usb_image_smoke.py`), the stick is swapped for a copy with another manifest while `vfs_server` makes no request. The copy is unchanged afterwards, and `vfs_server` logs `CAME BACK WITH ANOTHER MEDIUM`.

## Display output over several monitors (177; the maintainer's request, 2026-10-12)

**Recorded by:** the kernel track (KRN), 2026-10-12, for main task [177](177-display-output.md).

The maintainer asked for several monitors, their positions, and the GPU for the picture. 177 plans it in steps; this track's part is steps 3a–3c (`virtio-gpu` first, testable in QEMU with two outputs; then Intel's display engine from its PRMs; then AMD), 4 and 5. The kernel track does steps 0–1 (every output and its EDID, every screen the firmware lit, in `BootInfo`) first, and the interface versions the later steps need. Nothing here waits yet: number it when you start.


## The Ryzen 9 5900HX machine: sound, Ethernet, a second NVMe disk, a second xHCI (213)

**Recorded by:** the kernel track (KRN), 2026-10-13, from the maintainer's first run on an AMD machine ([213](213-amd-ryzen-pc.md); `log:hw0003.txt`).

### Problem

- **Sound:** `audio_gw` took the GPU's HDMI audio controller (05:00.1, 1002:1637, codec 1002:AA01: "NO OUTPUT PATH") and gave up. The analog controller (05:00.6, 1022:15E3, subsystem 1022:D270) was not tried.
- **Ethernet:** a Realtek RTL8168 (10EC:8168 at 02:00.0) has no driver.
- **A second NVMe disk** (04:00.0, 1F40:1202) has no driver: `nvme` drives the first (01:00.0).
- **The second xHCI** (05:00.4): `usb_host` instances, [213-KRN-0099](213-KRN-0099-a-usb-host-for-every-xhci.md).

### Plan (a proposal; the drivers track decides)

- **`audio_gw`:** try each HDA controller and use the first codec with an output path; a GPU's HDMI audio only when nothing else plays.
- **An RTL8168 driver** serving `idl/net.wit`, for the network on real hardware (550).
- **`nvme` instances** for further controllers, `init`'s half asked of the kernel track then.
- **`usb_host` and the class drivers** with more than one host controller.

### Acceptance criteria

On this machine a tone plays through its analog output, and the Ethernet card gets an address by DHCP.

## The USB keyboard on the Ryzen machine's xHCI types nothing (213)

**Recorded by:** the kernel track (KRN), 2026-10-13, from the maintainer's fifth boot of the Ryzen 9 5900HX machine ([213](213-amd-ryzen-pc.md); `log:boot0001.log`–`boot0005.log`).

### Problem

On boot 5 the shell was on the screen and typing did nothing. What the logs show:
- **The device:** 1C4F:000E, low speed, on port 4 of the first xHCI (05:00.3, AMD 1022:1639), slot 2.
- **`usb_hid`:** it took interface 0 as a boot keyboard (endpoint 81) and left interface 1 (consumer and system control, report IDs 1 and 2, endpoint 82) alone.
- **`usb_host`:** it polls slot 2 endpoint 3 (packet 8, interval 10). On boots 1–3 it logged a first report in the boot format, for example `[00, 00, 3B, 00, 00, 00, 00, 00]` (F2).
- **Not in the logs:** whether later reports came, and whether `usb_hid` turned any of them into key events. The internal PS/2 controller also exists (`ps2_kbd` runs, "NO MOUSE"), so the shell's keyboard service is `ps2_kbd`.

The xHCI interrupt endpoints of the MacBook Pro's full-speed mouse (Intel xHCI) work; this is a low-speed device on an AMD xHCI.

### Plan (a proposal; the drivers track decides)

- **Count it:** `usb_host` logs, once a second at most, the reports and completion codes of each interrupt endpoint after the first, and `usb_hid` the key events it sends.
- **Low speed on xHCI:** check the endpoint context (the interval in 125 µs units for a low-speed device, Max ESIT Payload 8, Max Packet Size 8) and the re-queued transfer after each completion.
- **The maintainer's next run:** the same keyboard on another port or behind a hub, and another keyboard.

### Acceptance criteria

On this machine the keyboard types in the shell, and the counters above are in the log.
