# Requests for the drivers track (DRV), not numbered yet

**Owner:** drivers track (the assessing session, `claude/ASR-DRV`, TRACKS 1.5) · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-09

The request on transfers to a gone device became [211-DRV-0021](211-DRV-0021-a-gone-device-fails-at-once.md) (2026-10-09). The network track's request that `bcm_wifi` names OFDM in `idl/wlanhw.wit` 1.1 became plan item 2 of [550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md) (2026-10-14).

The drivers track numbers its own tasks (`NNN-DRV-MMMM`), so requests from other tracks wait here. The drivers track turns each into a task and removes it from this file, and the file goes when it is empty.

## BCM4331 transmit status must check its deadline on every path

**Recorded by:** ASTRA auditor-fixer, 2026-10-16, for the maintainer's original
MacBook network-stability report. **Priority:** P1. **Roadmap:** D / 550.
**Constitution:** MC-6.1, MC-12.1, MC-12.3. Related owner task:
[550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md).

At owner branch `5623c9ba`, now integrated into ASTRA `ac1854f2`,
`bcm_wifi/src/radio.rs::Radio::status` continues after every available
intermediate or wrong-cookie TX status before reaching its 100 ms deadline
check or RX poll. Continuously available statuses can bypass that bound;
a final matching status is accepted even if the clock has passed the end.
The radio service is synchronous, so its caller's IPC timeout does not stop
this loop. [Audit and evidence boundary](../issues-audit/2026-10-16-bcm-tx-wait-audit.md).

Please number and implement a bounded production wait on every path,
preserving the existing status format and 100 ms budget. Exercise continuous
unrelated/intermediate statuses, no status, success before the deadline and
a matching status observed after it. Verify the transmit-slot ownership on
timeout before allowing reuse; a missing TX result is not completion.
Publish the built owner commit to fast-test before tests, and record the
original MacBook's ping-under-load/recovery run separately. This finding
does not yet establish the hardware cause of the reported packet loss.

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

## The output volume and the microphone's sensitivity in `audio.wit` (000-APP-0076, 000-APP-0077)

**Recorded by:** the tools track (APP), 2026-10-10, for [000-APP-0048](000-APP-0048-wm-settings.md), and widened on 2026-10-14 at the maintainer's request: "the output volume and the microphone's sensitivity need settings". The tools track's parts are [000-APP-0076](000-APP-0076-output-volume.md) (the volume) and [000-APP-0077](000-APP-0077-microphone-sensitivity.md) (the microphone).

### Problem

`audio.wit` 1.1 plays, records and makes tones, but has no level:

- **Output.** `audio_gw` leaves AC'97's master volume at 0 dB and its PCM output at the default, and unmutes HDA's output amplifiers at 0 dB. A program can only scale its own samples.
- **Capture.** It fixes the gain at start: AC'97's microphone at 0 dB without its +20 dB boost and the record gain at 0 dB (`audio_gw/src/main.rs`:64), HDA's input amplifiers at 0 dB.
- **So** neither the shell nor `wm`'s Settings can set how loud the sound is or how sensitive the microphone is.

### Plan (a proposal; the drivers track decides)

- **`audio.wit` 1.2:**
  - `levels: func() -> levels`: the output's level in per cent and muted or not, the capture's gain in per cent and the boost on or off, each with its range in dB where the device has one (none: scaled in `audio_gw`, or absent);
  - `set-volume: func(percent: u8, muted: bool) -> result<_, error-code>`;
  - `set-gain: func(percent: u8, boost: bool) -> result<_, error-code>`.
- **On the device:**
  - AC'97: the master volume (0x02) and the PCM output (0x18); the record gain (0x1C), and the microphone's boost (0x0E bit 6);
  - HDA: the output path's amplifier gain and mute; the input path's amplifier, the ADC's gain and the pin's boost where the codec has them; the digital microphone's gain on Apple's Cirrus codecs, if their coefficients have one;
  - where the device has no amplifier, `audio_gw` scales the samples, and `levels` says so.
- **Authority** (MC-3.11, MC-11.5):
  - `set-volume` and `set-gain` take a client with a badge that `init` gives the shell alone (requests-KRN.md, "The shell's audio client that may change the levels");
  - any other client gets `rights`;
  - `levels` answers every client.
- **Not kept by `audio_gw`.** The shell keeps the levels in `data/sound.conf` and sets them again when it starts (000-APP-0076).

### Acceptance criteria

- **The `audio` suite** (AC'97 and HDA):
  - `set-volume(50)` lowers the level QEMU's audio backend records for the same tone, by about 6 dB, or by what the codec's step allows;
  - `set-gain` reads back through `levels`, and raises the captured level where QEMU's capture has an input;
  - a client without the badge gets `rights`.
- **On the MacBook Pro:** the speaker's volume follows `set-volume`, and the internal microphone's level follows `set-gain`.

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
With the same build, a low-speed keyboard (046D:C31C) typed on the Intel Core i5 PC's xHCI (8086:8C31; [214](214-intel-core-i5-pc.md)). The difference is the AMD xHCI or this keyboard.

**Possibly not a driver's fault (2026-10-13, evening):** the shell on this machine never got past reading COM1, which it does not have (its ports read all ones, "a byte is ready" for ever). That is the black screen of [213-KRN-0098](213-KRN-0098-black-screen-when-the-shell-takes-it.md), fixed in the kernel track's `Uart::open` and `init`. A shell stuck there takes no key either. Check it again after the maintainer's next run with that fix.

### Plan (a proposal; the drivers track decides)

- **Count it:** `usb_host` logs, once a second at most, the reports and completion codes of each interrupt endpoint after the first, and `usb_hid` the key events it sends.
- **Low speed on xHCI:** check the endpoint context (the interval in 125 µs units for a low-speed device, Max ESIT Payload 8, Max Packet Size 8) and the re-queued transfer after each completion.
- **The maintainer's next run:** the same keyboard on another port or behind a hub, and another keyboard.

### Acceptance criteria

On this machine the keyboard types in the shell, and the counters above are in the log.

## The boot stick not set up on a USB 2 port of an Intel xHCI: no boot volume, no programs (214)

**Recorded by:** the kernel track (KRN), 2026-10-13, from a photo of the screen of the maintainer's Intel Core i5-4430 PC ([214](214-intel-core-i5-pc.md)). The PC's logs from two later boots name its controllers.

### Problem

- **What happened:**
  - The bootloader started from the stick ("starting MIND Core from its own volume"), the services started and the keyboard typed.
  - Then `usb_host` logged `PORT 8: DEVICE NOT SET UP AT ADDRESS DEVICE (COMPLETION 1, PORTSC 00000E03, TRY 3 OF 3)`, then `NO MASS STORAGE DEVICE`.
  - `vfs_server` found the boot volume "ON NO BLOCK DEVICE: NONE MOUNTED". So no program could start (`wm`: unknown command, `ls`: not found), and no log or hardware report was written.
- **The controller:** the H81's xHCI (8086:8C31) has 17 ports. Ports 1–10 are USB 2 and ports 11–15 are disabled. Ports 16–17 are USB 3. The kernel moved every port from EHCI to it ("USB 2 FFF OF MASK FFF, USB 3 3 OF MASK 3"), so the routing is not the cause.
- **PORTSC 0x00000E03:** connected, enabled, U0, powered, high speed. The stick is a SuperSpeed one (7825:A2A4) that came up at high speed on port 8, a USB 2 port.
- **Not every time:** on boot 1 (`log:boot0001.log`) the same stick was set up on the same port 8 at high speed, with the same build. On boot 2 it was in a blue port and was set up at SuperSpeed on port 16.
- **The completion code shown is not Address Device's:**
  - `enumerate` gives up at "ADDRESS DEVICE", and `discard` then disables the slot.
  - That Disable Slot command overwrites `xhci.last`.
  - So the line shows Disable Slot's success (1), not why Address Device failed.

### Plan (a proposal; the drivers track decides)

- **Keep the failing command's code** for the line: save it before `discard`, or log it in `enumerate`.
- **A high-speed device on an xHCI USB 2 port:**
  - the 10 ms reset recovery (USB 2.0 TRSTRCY) before Address Device;
  - EP0's 64-byte packet;
  - an Address Device with BSR=1 first, then without, where a device needs the time.
- **The maintainer's next run, after that:** several boots with the stick in a black port, from power-off and by `reboot`, until the line shows Address Device's own code.

### Acceptance criteria

- The stick is set up on that PC's USB 2 ports.
- When Address Device fails, the line names its own completion code.

## The Intel Core i5 PC: sound and Ethernet (214)

**Recorded by:** the kernel track (KRN), 2026-10-13, from the maintainer's two boots of that PC ([214](214-intel-core-i5-pc.md); `log:boot0001.log`, `boot0002.log`, `hw0001.txt`, `hw0002.txt`).

### Problem

- **Sound:**
  - `audio_gw` took the processor's display audio (00:03.0, 8086:0C0C: 0 input and 2 output streams), and its codec did not answer ("CORBRP 1 RIRBWP 0"). The screen is on a GeForce GT 1030, so the processor's display, which that codec depends on, is likely off.
  - It did not try the analog controller (00:1B.0, 8086:8C20, subsystem 1462:D817).
  - The GT 1030's HDMI audio (01:00.1, 10DE:0FB8) is a third controller.
- **Ethernet:** a Realtek RTL8168 (10EC:8168, revision 0C, at 03:00.0) has no driver.

### Plan (a proposal; the drivers track decides)

This is the Ryzen machine's request above, with one addition:
- **`audio_gw`:** pass over a controller whose codec does not answer, as well as one with no output path.
- **The RTL8168 driver:** this revision as well.

### Acceptance criteria

- On this PC a tone plays through its analog output.
- Its Ethernet card gets an address by DHCP.

## `ata` and `ahci`: waits with a deadline, and checks that do not lean on the caller (the critique of 2026-10-14)

**Recorded by:** the kernel track (KRN), 2026-10-14, from the critique [issues-critica-done/02](../issues-critica-done/02_code_and_comliance_analysis.md), §2.1–2.2, checked against `main` dfc42ee.

### Problem

- **`ata/src/main.rs`:**
  - `command` (:46) and `flush` (:89) wait `while self.io.in8(COMMAND) & BSY != 0 {}` with no bound. A drive stuck busy hangs the driver, and every request of `vfs_server` with it (MC-6.1). The likeliest trigger is the next command after `wait_data` gave up part way through a transfer.
  - `probe` (:26), `wait_ready` (:38) and `wait_data` (:55) count port reads instead of time. Each read is a system call, so how long they wait depends on the machine (MC-5.6). `mind::time::monotonic_ns` is there, and `nvme`'s `wait_ms` and `usb_host`'s xHCI already wait to a deadline.
  - `count as u8` (:48) and `chunks_exact(512).take(count)` (:71, :81) would go wrong for a `count` of 0 (which ATA reads as 256 sectors), above 256, or longer than the buffer. Today `block_protocol::span` holds every request to 1–128 sectors and a buffer of exactly that size, so nothing reaches it. The comment at :67 ("up to 256 sectors") is stale.
- **`ahci/src/main.rs`:**
  - The BIOS/OS handoff (:32) ignores what `wait` returns, and does not wait for BOHC.BB (BIOS busy) to clear, as the AHCI specification's handoff procedure asks. Where the firmware keeps the controller, the driver programs it anyway after about 10 s (MC-1.5, MC-6.3).
  - `read` and `write` (:87–90) slice the DMA region at `DATA` (64 KiB) plus `count × 512` with no check. That holds only because `BLOCK_MAX_SECTORS` (128) and `init`'s `AHCI_DMA_BYTES` (128 KiB) happen to fit together. A change to either makes the driver panic (MC-2.4).
  - `wait` (:21) spins 1000 times, then sleeps in 10 ms steps. On x86 the tick is 1 ms since 000-KRN-0083, so a slow completion costs up to 10 ms more than it needs.
- **`usb_host/src/xhci.rs:53`**: its comment says "a sleep is a 10 ms tick". On x86 it is 1 ms now.

### Plan (a proposal; the drivers track decides)

- One deadline helper in each driver, on `monotonic_ns`, for every wait. `command` and `flush` return an error on timeout, and `read` and `write` pass it on as `ERR_IO`.
- Each `read` and `write` checks `count` against the DMA region and the buffer itself, and refuses rather than panics.
- `ahci`: log the handoff's result; wait for BB; if the firmware keeps the controller, leave it and say so.
- `ahci`'s `wait` sleeps in 1 ms steps to its deadline, as `nvme` does.

### Acceptance criteria

- The `vfs`, `disk` and `block` suites pass on IDE and AHCI disks.
- A request on a drive that never leaves BSY ends with `ERR_IO` within the stated time. A host test with a stand-in for the ports, or a QEMU disk made to stall, shows it.
- No path in either driver indexes the DMA region or the buffer without a check of its own.

## BCM4331 applies the selected country and channel policy

**Recorded by:** ASTRA auditor-fixer, 2026-10-16, at the maintainer's explicit
country-switching request; roadmap D / 550; MC-2.3, MC-6.1, MC-12.1, MC-12.3.

The original Intel MacBook and Ryzen are physically in Uzbekistan (`UZ`). The
maintainer wants switching among all countries in the verified regulatory
catalogue. Please coordinate BCM4331's part with the
[NET policy/configuration request](requests-NET.md#wifi-country-selection-from-the-complete-verified-regulatory-catalog)
and [APP selector](requests-APP.md#wifi-country-and-channel-selector); ASTRA
implements the RTL8852BE catalogue portion in its existing DRV-0032 task.

Intersect the selected country's frequency/EIRP restrictions with the actual
BCM/SPROM/platform limits; do not treat a chip country table as the whole
regulatory decision. Keep unsupported constraints visible and ensure old
channel/TX decisions cannot survive a country-policy change. Coordinate the
versioned radio interface or restart boundary rather than silently changing an
existing wire contract. Preserve normal association, software WPA2 and the
original MacBook path; validate actual hardware separately from policy fixtures.
This request does not replace the preceding H12 deadline correction.
