# 550-DRV-0006 — The MacBook Pro's Broadcom Wi-Fi

**Type:** driver · **Owner:** `DRV` (open) · **Priority:** P3 · **Status:** open (proposed; steps 1 and 2 come first) · **Blocked by:** the PCI ID (step 1); the firmware licence (step 2); the 802.11 station from `NET` ([requests-NET.md](requests-NET.md)) for anything past a scan · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-12.1, MC-1.5, MC-2.3

Numbered by the kernel session at the maintainer's request (2026-10-08); the track is open.

## Problem

`04:00.0` on the MacBook Pro is a network controller (class `028000`) with no driver. Apple's specifications give a Broadcom BCM4331 (802.11n) for this model. The PCI ID has not been read, so the chip is still to be confirmed.

If it is a BCM4331, three things make it hard:

- **SoftMAC.** The chip does not run the 802.11 MAC layer itself. The host builds and parses management frames, scans, authenticates, associates and runs WPA2. That layer belongs to the network track (the 802.11 station request), not to this driver.
- **Firmware.** The chip's 802.11 core runs microcode the host loads at every start. That microcode comes from Broadcom's own drivers: Linux's `b43` driver does not ship it, and its users extract it with `b43-fwcutter`. Whether MIND Core may ship it, or may only load a file the user provides, is a licence question.
- **Documentation.** Broadcom publishes no programming manual for the chip. What exists is Linux's `b43` and the specifications its developers wrote by reverse engineering. `b43` is GPL: its code is read for behaviour, not copied. How far either covers this chip's PHY and radio is part of step 3.

## Plan

1. **Read the PCI ID** on the Mac: `lspci -nn` from a Linux live USB stick, or System Information under macOS (`devices` prints classes only). A BCM4331 is expected as `14E4:4331`. If it is another chip, this task is rewritten for it.
2. **The firmware licence:** which files the chip needs, where they come from and under what terms. The question goes to the maintainer through [issues-human](../issues-human/README.md) before any firmware file enters the repository or an image. Until it is answered, the driver loads firmware only from a file the user puts on the disk, and nothing is shipped. Whatever is shipped later is recorded in [THIRD_PARTY.md](../THIRD_PARTY.md).
3. **A ring-3 driver** (`bcm_wifi`, proposed name): the chip's internal backplane and its cores behind the PCIe function, core reset, the firmware upload, PHY and radio set-up, the DMA rings for transmit and receive, interrupts. It does DMA without an IOMMU, so it joins the TCB of memory isolation (MC-1.5), and [docs/profile/tcb.md](../docs/profile/tcb.md) says so when it lands.
4. **An interface to the station:** raw 802.11 frames, the channel, and the keys for the chip's own encryption if it is used. A new IDL file, agreed with `NET`, which owns the station (MC-2.3).
5. **Scan** through the station: probe requests out, beacons and probe responses in.
6. **Association and data** with the station's WPA2-PSK.

Steps 3–6 are large. Nothing here is promised by a date.

## Acceptance criteria

- The PCI ID and the answer on the firmware are recorded in this issue.
- First check, on the Mac: the networks around it are listed with their SSID, channel and signal. QEMU has no Wi-Fi device, so no CI suite covers the chip.
- Then, with the station and the setup program: the Mac joins a WPA2-PSK network and `https` fetches a page (550's Wi-Fi criterion).

## Related

[550](550-network-on-real-hardware.md), [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md), [requests-NET.md](requests-NET.md) (the 802.11 station), [550-APP-0033](550-APP-0033-wifi-setup-program.md) (the setup program), [THIRD_PARTY.md](../THIRD_PARTY.md).
