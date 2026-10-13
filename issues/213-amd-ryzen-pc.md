# 213 — An AMD Ryzen PC: the maintainer's Ryzen 9 5900HX machine

**Type:** main task, porting · **Owner:** `PRT` (kernel session), with `KRN` and `DRV` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** track H · **Constitution:** MC-12.1, MC-12.3

## Problem

The first run on an AMD machine (2026-10-13, `fast-test` 28d9c3d, three boots, `log:boot0001.log`–`boot0003.log`, `log:hw0001.txt`–`hw0003.txt`): an AMD Ryzen 9 5900HX (Zen 3, 8 cores, 16 threads, family 0x19 model 0x50), 32 GB, a Radeon Vega iGPU (1002:1638) driving a 2560 × 1440 screen through the firmware's GOP (BGR, stride 2560, at 0xD0000000 in the GPU's BAR0), AMI firmware with an AMD IOMMU (IVRS) and no battery: a desktop or mini PC.

**What worked:**
- the bootloader, the signed manifest and the kernel: 16 CPUs, xAPIC, SMEP, SMAP and UMIP, the LAPIC tick measured on the ACPI PM timer, a 32000 MiB frame pool;
- every boot service started, the shell too (PID 28);
- the boot stick on the first xHCI (05:00.3), the USB keyboard 1C4F:000E, the first NVMe disk (1ED0:2283), the log partition: each boot's log and hardware report were written.

**What did not:**
- **the screen:** the white boot lines showed, then the screen went black when the shell should have shown its green text ([213-KRN-0098](213-KRN-0098-black-screen-when-the-shell-takes-it.md));
- **sound:** `audio_gw` took the GPU's HDMI controller (05:00.1, codec 1002:AA01, no output path), not the analog one (05:00.6, 1022:15E3), which it did not try;
- **the second xHCI** (05:00.4) and **the second NVMe disk** (04:00.0, 1F40:1202) have no driver;
- **network:** a Realtek RTL8168 Ethernet card (10EC:8168) and an RTL8852BE Wi-Fi card (10EC:B852, its Bluetooth on USB as 0BDA:B85B) have no driver.

## Plan

1. The black screen first: [213-KRN-0098](213-KRN-0098-black-screen-when-the-shell-takes-it.md).
2. Every xHCI controller: [213-KRN-0099](213-KRN-0099-a-usb-host-for-every-xhci.md) (`init`) with the drivers track's `usb_host`.
3. The drivers track's part, asked in [requests-DRV.md](requests-DRV.md): the analog HDA codec, the RTL8168 Ethernet card, the second NVMe disk.
4. Each run's evidence is for this machine and this build only (MC-12.1).

## Acceptance criteria

The shell shows on this machine's screen; its keyboard, its first NVMe disk and its log partition work; sound plays through its analog output; each finding above is a closed task or a numbered one.

## Related

[211](211-intel-pc-from-a-sata-ssd.md) (the first real x86 PC), [174](174-full-use-of-pc-hardware.md) (the PC's hardware, AMD included), [requests-DRV.md](requests-DRV.md).
