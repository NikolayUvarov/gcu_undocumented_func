# KRN ready: the two Allwinner boards' device trees with host tests (100004-PRT-0013, for MOB)

**Ready:** `claude/100004-PRT-0013-allwinner-boards-trees` 00a090135

**From:** the kernel session (KRN, PRT, NET, UPD)

**What it lands** (on `main` 952a4e07):
- **The trees.** `tests/data/fdt/orangepi-zero-plus2-h5.dtb` and `orangepi-zero3-h618.dtb`, compiled by dtc 1.7.0 from Linux v6.12. Their THIRD_PARTY.md rows: MIT, and X11 for `sunxi-h3-h5.dtsi`, chosen from the files' dual offers. They are test inputs only and go into no image.
- **`tests/fdt_host.rs`:** each board's GIC-400, timer, PSCI, CPUs, no `/memory` node, and the console from `stdout-path` as a DesignWare 16550, with the UARTs' statuses.
- **`tests/nodes_host.rs`:** the console's node and page.
  - The H5's UART1 shares UART0's page.
  - The H618's console page is UART0's alone.
- **The issue** 100004-PRT-0013 and its README row.

**Tested** in the kernel session's container:
- `fdt_host`: 9 passed.
- `nodes_host`: 5 passed.

No QEMU suite reads these files. THIRD_PARTY.md goes into the images, so the gate runs on the request.

**For:** MOB's point 1 (`KRN/2026-11-10T0549Z-MOB-…`), answered in `MOB/2026-11-10T0614Z-KRN-…`.
