# 100004-MOB-0002 — The first boot on the Orange Pi Zero Plus 2 H5

**Type:** porting (a board) · **Owner:** `MOB` · **Priority:** P1 · **Status:** open · **Blocked by:** [100004-MOB-0001](100004-MOB-0001-the-boards-sd-card.md) (the card); the kernel leaving EL2 (asked of `KRN`, 2026-11-10); the board wired to a host ([issues-human, section 14](../issues-human/README.md#14-the-two-allwinner-boards-on-the-bench)); for the console past the kernel's lines, the kernel session's answer on the busy-detect and the console channel (100004's message to `KRN`) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-1.3, MC-12.1, MC-12.3

## Problem

MIND Core's aarch64 image has run only in QEMU. The H5 board is the nearest thing to the A64 at hand. Its first boot shows what the port assumes that a real Allwinner board does not give:
- the UEFI map and the tree as U-Boot gives them;
- the GIC-400 and PSCI from TF-A;
- the DesignWare UART at shift 2, width 4;
- 512 MiB;
- no disk the system drives once the loader has run (100004-MOB-0005).

## Plan

1. **On the host first:** the board's device tree (Linux's `sun50i-h5-orangepi-zero-plus2.dts`, as U-Boot installs it) through `common/fdt.rs`'s host tests: the GIC, the timer, the console, PSCI, the memory. That is the kernel session's (`PRT`); asked in 100004's message.
2. **In QEMU, the nearest stand-in:** `virt` under U-Boot's UEFI, 4 × Cortex-A53, GICv2, 512 MiB (the group "aarch64: the board from a device tree, under U-Boot", with `-m 512`). This only shows that the image fits in the memory.
3. **On the board,** over UART0 at 115200, recorded line by line:
   - U-Boot's banner and version, and our loader's lines;
   - the kernel's `BOARD` line: the GIC, the UART, the CPUs, the memory;
   - every CPU through PSCI, the timer's ticks, the shell or `link`;
   - a reboot through PSCI.
4. **What fails** goes to its owner as a request: the kernel to `PRT` and `KRN`, `init` to `KRN`, a driver to `DRV`.

## Acceptance criteria

- **The board boots to the shell or `link`,** with four CPUs, and reboots on command.
- **Its log is kept** in this issue's progress, with:
  - the board's revision;
  - U-Boot's and TF-A's versions;
  - the card;
  - the adapter and the host (MC-12.1).

## Progress (2026-11-10)

**Found before the board: entered at EL2, the kernel stops at `init`'s first system call.**
- **Why EL2:** TF-A hands over to U-Boot at EL2, and U-Boot's defconfigs for both boards do not set `ARMV8_SWITCH_TO_EL1`. So `BOOTAA64.EFI` and the kernel start at EL2.
- **What the code has:** nothing in `bootloader/src` or `kernel/src/arch/aarch64` reads `CurrentEL` or drops to EL1.
- **Reproduced in QEMU** (8.2.2, TCG): the U-Boot group with `virtualization=on` added (`--machine virt,acpi=off,gic-version=2,highmem=off,virtualization=on`).
  - The loader, the kernel's `BOARD` line and `INIT STARTED` come as usual.
  - Then U-Boot's handler reports `"Synchronous Abort" handler, esr 0x56000000`: an SVC taken to EL2, where `VBAR_EL2` is still U-Boot's. The machine resets.
- **Without `virtualization=on`** the same tree passes: `virt` has no EL2 then.
- **Asked of the kernel session** (`KRN/2026-11-10T0700Z-MOB-entered-at-el2-…`): the drop to EL1 for every CPU, and a CI group with `virtualization=on`.

**Without a GOP** (the H618's case), under U-Boot in QEMU, the system boots to `MIND>` and starts programs. The `normal` suite then fails only on its expectation of `PID=1` for the first program.

**The card** for this board is 100004-MOB-0001's `dist/boards/mind-orangepi-zero-plus2-h5.img`.

## Related

[100004-MOB-0003](100004-MOB-0003-the-first-boot-on-the-h618.md), [100000-PRT-0010](100000-PRT-0010-a-16550-uart-in-memory.md), [100000-KRN-0222](100000-KRN-0222-the-kernels-console-as-a-byte-channel.md), [100000](100000-a-phone-the-pinephone-first.md) (stage 1).
