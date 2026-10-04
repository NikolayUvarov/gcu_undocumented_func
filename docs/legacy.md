# Legacy hardware support

**Version:** 0.1 (2026-10-04)

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
| Primary IDE channel, PIO (ports 0x1F0–0x1F7, 0x3F6) | the `ata` crate, its plan in `init`, its ranges in the kernel's `LEGACY_PORTS` | AHCI (`ahci`), later NVMe and VirtIO block | remove `ata` from `BOOT_SERVICES`/`BOOT_FILES`, its `init` plan and build entry, the two port ranges | counted: "IDE CONTROLLER" (PCI class 01:01) |
| AC97 audio | `audio_gw` (the only audio driver), its `init` plan | Intel HD Audio or VirtIO sound (not written yet) | replace the device code in `audio_gw` | counted: "AC97 AUDIO" (PCI class 04:01) |
| PS/2 keyboard controller (ports 0x60, 0x64, IRQ 1) | the `ps2_kbd` crate, its `init` plan, its ranges in `LEGACY_PORTS` | USB HID, VirtIO input (not written yet; the shell also takes keys from the UART) | as for `ata` | assumed (ISA) |
| CMOS RTC (ports 0x70–0x71) | the `rtc` crate, its `init` plan, its range in `LEGACY_PORTS` | UEFI runtime time services or an ACPI time device | as for `ata`; clients use `idl/rtc.wit` and keep working with a new server | assumed (ISA) |
| COM1 UART (ports 0x3F8–0x3FF) | the shell's console (`shell/src/console.rs`), the kernel's boot and panic lines | VirtIO console | the kernel's diagnostics need another output first | assumed (ISA) |
| 8259 PIC and PIT (device lines 1–15, the 100 Hz tick) | `kernel/src/interrupts.rs`, the line handling in `kernel/src/scheduler.rs` | IOAPIC or MSI/MSI-X for devices (MSI-X exists: issue 104), the local APIC timer | the largest item: every driver on a legacy line needs MSI/MSI-X or the IOAPIC first | assumed |
| PCI configuration mechanism #1 (ports 0xCF8/0xCFC) | `kernel/src/pci.rs` | ECAM, found through the ACPI MCFG table | needs ACPI table parsing in the kernel | not reported (always present on x86 today) |

The legacy interrupt line of a modern VirtIO card (`MODERN INTX` when MSI-X cannot be set up) belongs to the PIC row.

## Rules

- New code for a superseded interface is accepted only with a `LEGACY:` marker at every place it touches, a row here, and, where the device can be detected, a check in `init/src/legacy.rs`.
- Isolate it so removal is a deletion: its own file or crate, a cargo feature where the rest of a driver is modern.
- Remove a row when its last user is gone; the boot report tells when no tested machine needs it any more.
