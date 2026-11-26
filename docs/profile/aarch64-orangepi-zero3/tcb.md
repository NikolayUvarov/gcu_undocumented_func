# Trusted computing base — `aarch64/OrangePi-Zero3-0` (a skeleton, not tested)

What each guarantee would rest on when MIND Core runs on the Orange Pi Zero 3 (H618). Read in the code and in the firmware's sources; **none of it has run on the board** (MC-12.3). The kernel, its architecture layer and the services are those of [`aarch64/QEMU-virt-0`'s TCB](../aarch64/tcb.md); this page says what the board puts in their place or beside them.

| Guarantee | TCB on this board | Status |
|---|---|---|
| Every guarantee | **The firmware:** the H618's boot ROM; U-Boot's SPL, which sets the AXP313a's DRAM rail and sets up the DRAM; TF-A's BL31, which stays resident at EL3 as PSCI and the secure monitor; U-Boot and its UEFI until `ExitBootServices`. None is checked by our keys; U-Boot and TF-A are built from the pinned sources of `scripts/boards.toml` by `scripts/board_uboot.sh` | not tested |
| Memory isolation between tasks; kernel integrity | The CPU (MMU, exception levels); the EL2 state the kernel leaves (100004-PRT-0014); the bootloader and the kernel. No driver with DMA runs yet; the MMC, USB and Ethernet drivers, when they come, join this TCB, as the IOMMU the tree names is not driven (MC-1.5) | not tested |
| CPU start, reset and power off | PSCI as TF-A's BL31 implements it, by SMC | not tested |
| Interrupt delivery | The GIC-400 (GICv2) without a GICv2m frame | not tested |
| The board's layout | U-Boot's device tree, believed for where each device is; the kernel's reader (`common/fdt.rs`, `arch/aarch64/devicetree.rs`) | host tests of Linux's tree only |
| The console | The kernel's 16550 code and its byte channel (100000-KRN-0222): `init` lends a console's node only when it is a PL011, so no task gets the UART's registers; the shell holds the channel | host tests only (100000-PRT-0010) |
| A frozen machine restarts | The A31's watchdog and the kernel's feeding of it (100004-PRT-0015) | not tested |
| The power rails | The AXP313a's outputs as U-Boot's SPL left them: the system does not touch the PMIC until 100004-MOB-0007 | not tested |
| Calendar time; the device key; TLS | Not provided here: no RTC driver for this SoC, no RNDR | — |
| The bench's components | *to be named* (README, "The bench's components") | — |
