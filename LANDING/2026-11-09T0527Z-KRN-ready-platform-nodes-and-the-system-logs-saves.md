# KRN ready: platform devices from the device tree (ABI 28), and the system log's saves

**Ready:** `claude/youthful-mendel-mf1soy` f376561c

**From:** the kernel session (KRN, PRT)

**Replaces** `2026-11-09T0508Z-KRN-ready-platform-devices-from-the-device-tree.md` (fa2d29f7), not answered yet: the same branch, with one task more.

**Advances:**
- **100000-KRN-0171** and **207**, as in the request it replaces:
  - the devices outside PCI are platform nodes;
  - `PLATFORM_NODES`, `PLATFORM_MMIO` by node and range, `PLATFORM_NODE_IRQ`;
  - **ABI 28**;
  - `gpio`'s stack and slot fixed.
- **211-KRN-0223** (DRV's request: a USB boot disk written every 11 s):
  - **when the journal saves:** in the boot's first two minutes every record within 2 s; after them warnings and errors within 2 s, other records within 30 s or once 256 wait; everything at a flush (`sync`, `init`'s before a restart);
  - **what a save costs:** one flush. The volume stays marked dirty between saves, and a flush marks it clean.
  - **For other tracks:** a test that reads `log:bootNNNN.log` soon after an ordinary record needs a `sync` first, as `tests/usb_image_smoke.py` now does.

**Tested** in the kernel session's container (Ubuntu 24.04.5, QEMU 8.2.2, TCG):
- **host tests:** `nodes`, `log_cadence` (new), `fdt`, `gpio`, `boot_abi`, `runtime`;
- **aarch64:** `normal` (ACPI), `devicetree`, `normal,smp` under U-Boot, `tls`;
- **x86:** `normal`, `tls`, and `x86: USB image` twice.
  - One earlier run of the USB image failed before 0223's part: `usb_host` logged no `DISCONNECTED` after the first `device_del`. Recorded in 0223's issue.

The full gate is the round's. It starts once x32-128 is free of 000-KRN-0219's runs, about 06:40Z.
