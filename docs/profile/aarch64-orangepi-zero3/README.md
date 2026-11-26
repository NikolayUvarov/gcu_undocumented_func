# Platform profile `aarch64/OrangePi-Zero3-0` — a skeleton, nothing tested

**Version:** 0.1 (2026-11-26), a skeleton · **Issue:** [100004-MOB-0013](../../../issues/100004-MOB-0013-the-boards-profiles-as-skeletons.md) · **Main task:** [100004](../../../issues/100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** [v1.6](../../../constitution/EN/MIND_CORE_Constitution_v1.6.md), stage 0 · **Roadmap:** track H

**No MIND Core has run on this board yet.** This profile is the place where what MIND Core is, guarantees and does not claim on the Orange Pi Zero 3 (H618, 2 GiB) will be stated (MC-12.1, MC-12.3). Today every statement below is **not tested**: it was read in a source named beside it, or in this repository's code, and has not been seen on the board. A line changes to tested only with a run on the board named in [evidence.md](evidence.md), with its configuration (the board's revision, the card, U-Boot's and TF-A's versions, the adapter, the host).
- **Evidence from QEMU does not carry over.** The runs of [`aarch64/QEMU-virt-0`](../aarch64/README.md), including `virt` under U-Boot's UEFI with four Cortex-A53, are evidence for QEMU only (MC-12.1).
- **Where this profile says nothing, it claims nothing.** It is not a list of differences from QEMU `virt` that inherits the rest.

| Document | Content |
|---|---|
| [tcb.md](tcb.md) | The trusted computing base as the code and the firmware make it here: not tested |
| [threat-model.md](threat-model.md) | What differs from `aarch64/QEMU-virt-0`'s threat and fault model on a bench board |
| [evidence.md](evidence.md) | Which run on the board supports which statement: none yet |

## The machine

| | What | Read in | Status |
|---|---|---|---|
| Board | Orange Pi Zero 3, 2 GB | the maintainer's handoff ([100004](../../../issues/100004-two-allwinner-boards-before-the-phone.md)) | not checked on the board |
| SoC, cores | Allwinner H618 (the `sun50i-h616` family), 4 × Cortex-A53: no PAN, no RNDR | [scripts/boards.toml](../../../scripts/boards.toml); the ARMv8.0 core | not tested |
| Memory | 2 GiB LPDDR4 at 0x40000000, taken from the UEFI map; TF-A keeps 0x40000000 to 0x4003FFFF; the tree has no `/memory` node, U-Boot adds one | the handoff; TF-A's sources (100004-MOB-0001); [100004-PRT-0013](../../../issues-done/100004-PRT-0013-the-allwinner-boards-trees.done) | not tested |
| Firmware | U-Boot v2026.10 and TF-A v2.15.0's BL31, on the card at 8 KiB; no network boot, the countdown stopped only by `stop` | `scripts/boards.toml`, `scripts/board_uboot.sh` ([100004-MOB-0001](../../../issues/100004-MOB-0001-the-boards-sd-card.md)); U-Boot's SPL sets the AXP313a's DRAM rail (1.1 V) before the DRAM | built, two builds the same bytes; not booted |
| Boot | U-Boot's UEFI starts `BOOTAA64.EFI` from the card's ESP, at EL2; the kernel goes on at EL1 (100004-PRT-0014) | U-Boot's and TF-A's sources; [100004-MOB-0003](../../../issues/100004-MOB-0003-the-first-boot-on-the-h618.md) | not tested |
| Device tree | `allwinner/sun50i-h618-orangepi-zero3`, U-Boot's copy of Linux's tree, installed as the EFI device tree | the defconfig at the tag | not seen; the host tests read Linux v6.12's tree of that name |
| Interrupts | GIC-400 (GICv2), distributor 0x03021000, CPU interface 0x03022000 | `tests/fdt_host.rs` on Linux's tree | host test of the tree only |
| Timer | the virtual timer, PPI 11 (line 27); no counter erratum | the same | host test of the tree only |
| CPUs, reset | four, started through PSCI by SMC (TF-A); `SYSTEM_RESET` for `reboot` | the same | host test of the tree only |
| Console | UART0, a DesignWare 16550 at 0x05000000 (shift 2, width 4), 115200, the 3-pin debug header; UART1 to UART5 share its page and are disabled; `init` lends a console's node only when it is a PL011, so the shell's line is the kernel's byte channel (100000-KRN-0222) | `tests/fdt_host.rs`, `tests/nodes_host.rs` | host tests only; the shell's console on a 16550 never run (100000-PRT-0010) |
| Watchdog | the A31's, the kernel's own (100004-PRT-0015) | the kernel's code | not tested on the board |
| Storage after the loader | none: no driver for the Allwinner MMC controller yet (100004-MOB-0005), so the card is not read once the loader has run; the board has no eMMC, and its SPI NOR flash is not driven | the code; the defconfig | not made |
| Network | none: no driver for its Gigabit Ethernet; its Wi-Fi and Bluetooth need drivers, and firmware only through `proprietary/` | the handoff | not made |
| USB | EHCI and OHCI not driven (100004-DRV-0067) | the code | not made |
| Screen | none: this board's U-Boot has no video (`CONFIG_VIDEO` off), so the loader starts without a screen | U-Boot's sources (100004-MOB-0001) | not tested |
| PMIC | the AXP313a on R_I2C (0x07081400): the system does not touch it (100004-MOB-0007) | the handoff; Linux's tree | not made |
| Calendar time | none: the tree names the SoC's RTC (`allwinner,sun50i-h616-rtc`), and `rtc` drives a PL031 only | Linux's tree; the code | not made |
| Entropy | no RNDR: no device key, no TLS (100000-KRN-0279) | the core | not tested |

## The bench's components

> **A marked place, to be filled.** The maintainer (2026-11-26): «MOB будет делать работу на стенде - это не телефон, а плата разработки с добавленными компонентами». Which board the bench is built on, and each component added to it with its bus, are to be named in [issues-human, section 14](../../../issues-human/README.md#14-the-two-allwinner-boards-on-the-bench). Each component then gets a row here, checked against the board's device tree, and stays "not tested" until a run.

| Component | Bus and address | Node in the device tree | Driver | Status |
|---|---|---|---|---|
| *to be named* | | | | |

## Conformance: what this board would change (none of it tested)

| Requirement | Status here | Why |
|---|---|---|
| MC-1.5 DMA boundary | not met, declared | The tree names an IOMMU (`allwinner,sun50i-h616-iommu` at 0x030F0000); MIND Core does not drive it, and which DMA masters it covers is not checked. No driver with DMA runs here yet; the MMC, USB and Ethernet drivers, when they come, are in the TCB of every memory guarantee. |
| MC-6.4–6.9 a frozen machine | not tested | The A31's watchdog resets the board 16 s after the boot CPU's tick stops (100004-PRT-0015): read in the code, not seen here. |
| MC-9.1 the boot chain | not tested | The boot ROM, U-Boot's SPL, TF-A and U-Boot are not checked by our keys and are trusted as they are. Our loader checks the signed manifest and every image it loads, as on `virt`. |
| MC-10.5 side channels | not claimed | No PAN on the Cortex-A53: the boot line is to say `PROTECTION: PXN`. |
| MC-12.1 evidence | none | No run on this board. |

## Not yet

Everything above, run on the board: the first boot (100004-MOB-0003), Effector over UART0 (100004-MOB-0004), the MMC controller (100004-MOB-0005), the AXP313a (100004-MOB-0007), the suites in 2 GiB (100004-MOB-0008). The steps of the first boot: [docs/boards/allwinner-first-boot.md](../../boards/allwinner-first-boot.md).
