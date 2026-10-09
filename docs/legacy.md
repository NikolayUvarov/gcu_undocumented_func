# Legacy hardware support

**Version:** 0.3 (2026-10-06)

MIND Core is built for current and future hardware. Support for superseded interfaces is kept only where a test machine or a supported platform still needs it, and it is kept **removable**: every place in the sources that exists only for a legacy interface is marked with a `LEGACY:` comment, and this page lists them all. `grep -rn "LEGACY:" --include=*.rs --include=*.toml .` must show nothing that is not in the table below.

At boot `init` reports what it finds (`init/src/legacy.rs`), for example:

```
[INIT] LEGACY VIRTIO DEVICE WITH ONLY THE LEGACY INTERFACE: NOT FOUND
[INIT] LEGACY IDE CONTROLLER: FOUND 1 (DRIVEN BY ata (PIO on ports 0x1F0))
[INIT] LEGACY AC97 AUDIO: NOT FOUND
[INIT] VIRTIO TRANSITIONAL DEVICES: 1 (LEGACY INTERFACE PRESENT, NOT USED)
[INIT] LEGACY ISA DEVICES, NOT ENUMERABLE, ASSUMED BY THE PLATFORM PROFILE: PS/2 KEYBOARD, CMOS RTC, COM1, PRIMARY IDE PORTS, 8259 PIC AND PIT
[INIT] LEGACY DEVICES FOUND: 1 (docs/legacy.md)
```

or `[INIT] NO LEGACY PCI DEVICES FOUND`. PCI devices are counted; ISA devices cannot be enumerated (no ACPI parsing yet), so they are listed as assumed.

## Components

| Component | Code | Replaced by | To drop it | Boot report |
|---|---|---|---|---|
| Legacy VirtIO PCI interface (I/O BAR0, queues at page frames) | `virtio_net/src/legacy.rs`, cargo feature `legacy` of `virtio_net` (default), `LEGACY:` lines in `virtio_net/src/main.rs` and the legacy branch in `init`'s `virtio_net` plan | the modern VirtIO 1.x interface with MSI-X (`mind::virtio`) | build `virtio_net` without the feature (`--no-default-features`: a legacy-only card is then reported and not driven), or delete the file, the feature, the marked lines and the `init` branch | counted: "VIRTIO DEVICE WITH ONLY THE LEGACY INTERFACE"; transitional devices are reported but need no legacy code |
| Primary IDE channel, PIO (ports 0x1F0–0x1F7, 0x3F6) | the `ata` crate, its plan in `init`, its ranges in `kernel/src/arch/x86_64/platform.rs` | AHCI (`ahci`), VirtIO block (`virtio_blk`, issue 202), later NVMe | remove `ata` from `BOOT_SERVICES`/`BOOT_FILES`, its `init` plan and build entry, the two port ranges | counted: "IDE CONTROLLER" (PCI class 01:01) |
| AC97 audio | `audio_gw` (beside Intel HD Audio, 551-DRV-0010, which wins where both are present), its `init` plan | Intel HD Audio (done, 551-DRV-0010) or VirtIO sound | remove the AC97 code from `audio_gw` | counted: "AC97 AUDIO" (PCI class 04:01) |
| PS/2 keyboard controller (ports 0x60, 0x64, IRQ 1; the mouse on its auxiliary port, IRQ 12, issue 156) | the `ps2_kbd` crate (`mouse.rs` for the mouse), its `init` plan, its ranges in `arch/x86_64/platform.rs` | VirtIO input (`virtio_input`: the tablet, issue 160, and the keyboard, issue 202) and USB HID (`usb_hid` over `usb_host`, issue 164), through the same decoder and keyboard service; the shell also takes keys from the UART. `init` does not start `ps2_kbd` on a machine without the controller (its status port reads 0xFF) | as for `ata` | assumed (ISA) |
| CMOS RTC (ports 0x70–0x71) | the CMOS half of the `rtc` crate (its PL031 half serves aarch64), its range in `arch/x86_64/platform.rs` | UEFI runtime time services or an ACPI time device | as for `ata`; clients use `idl/rtc.wit` and keep working with a new server | assumed (ISA) |
| COM1 UART (ports 0x3F8–0x3FF) | the 16550 half of `mind::dev::Uart` (the shell's console; the PL011 half serves aarch64), the kernel's boot and panic lines | VirtIO console | the kernel's diagnostics need another output first | assumed (ISA) |
| 8259 PIC and PIT (device lines 1–15; the 100 Hz tick only where the ACPI PM timer is missing, 211-PRT-0003) | `kernel/src/arch/x86_64/interrupts.rs`, `clock.rs`, the line handling in `kernel/src/scheduler.rs` | IOAPIC or MSI/MSI-X for devices (MSI-X exists: issue 104); the local APIC timer gives the tick | the largest item: every driver on a legacy line needs MSI/MSI-X or the IOAPIC first | assumed |
| PCI configuration mechanism #1 (ports 0xCF8/0xCFC) | `kernel/src/arch/x86_64/pcicfg.rs` | ECAM, found through the ACPI MCFG table (as on aarch64, `arch/aarch64/pcicfg.rs`, issue 202) | the x86 ACPI code reads the MCFG too, and the ECAM is mapped uncached | not reported (always present on x86 today) |

The legacy interrupt line of a modern VirtIO card (`MODERN INTX` when MSI-X cannot be set up) belongs to the PIC row.

## Rules

- New code for a superseded interface is accepted only with a `LEGACY:` marker at every place it touches, a row here, and, where the device can be detected, a check in `init/src/legacy.rs`.
- Isolate it so removal is a deletion: its own file or crate, a cargo feature where the rest of a driver is modern.
- Remove a row when its last user is gone; the boot report tells when no tested machine needs it any more.
