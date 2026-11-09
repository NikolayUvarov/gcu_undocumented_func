# KRN ready: platform devices are nodes of the device tree, lent by node (ABI 28)

**Ready:** `claude/youthful-mendel-mf1soy` fa2d29f7

**From:** the kernel session (KRN, PRT)

**Advances:**
- **100000-KRN-0171** (the PinePhone's stage 0): the devices outside PCI become platform nodes.
  - The kernel lists the device tree's enabled device nodes, or on a board with ACPI the UART, PL031, pin controllers and TPM its tables name; on x86, the TPM.
  - `PLATFORM_NODES` hands the list out; `PLATFORM_MMIO` lends a node's register pages, `PLATFORM_NODE_IRQ` its lines.
  - The fixed indices `PLATFORM_UART`, `_RTC`, `_PINS_*` and `_TPM` are gone: **ABI 28**. Every program is rebuilt with it.
  - A page another node's registers share is lent to none (the A64's four UARTs), nor RAM, nor the GIC's or the ECAM's registers.
  - `init` finds its drivers' nodes by compatible string.
- **207** (the porting track's `gpio`): its first start, on QEMU's PL061 from the tree, found main's frame past its 64 KiB stack and `SLOT_DEV1` = `SLOT_VFS`. It now asks 256 KiB, and its VFS client is in `SLOT_MEM`.

**For other tracks:** nothing of theirs changes. The fixed indices were used only by `init`, `gpio` and the kernel.

**Tested** in the kernel session's container (Ubuntu 24.04.5, QEMU 8.2.2, TCG):
- **host tests:** `nodes` (new), `fdt`, `gpio`, `boot_abi`, `runtime` (the ABI is 28);
- **aarch64:**
  - `normal` under AAVMF with ACPI (the console and `rtc` through their nodes);
  - `devicetree` (`acpi=off`: 35 nodes, AAVMF's disabled PL031 not listed);
  - `normal,smp` under U-Boot (`rtc` through the PL031's node, `gpio` driving the PL061);
  - `tls` (the TPM's FIFO through its node);
- **x86:** `normal`, and `tls` (the TPM's CRB through its node).

The full gate is the round's. The round waits until x32-128 is free: 000-KRN-0219's ten runs hold both its lanes until about 06:40Z.
