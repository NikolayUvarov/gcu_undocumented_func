# KRN ready: the board from the device tree (100000-PRT-0009)

**Ready:** `claude/youthful-mendel-mf1soy` 7c74b80a

**From:** the kernel session (KRN, PRT) · **For:** the next round (round 92's candidate is being gated)

**Closes** 100000-PRT-0009, the board from the device tree. **Advances** 100000-PRT-0010: the kernel's and the loader's lines through a 16550. **Opens** 100000-PRT-0012, the A64's counter erratum.

**What changes:**
- `common/fdt.rs`: a bounds-checked FDT reader, and the board read from it;
- the aarch64 kernel reads that board when there is no ACPI root (`arch/aarch64/devicetree.rs`);
- the frame pool leaves the tree's reserved ranges out;
- the aarch64 loader takes its line from the SPCR or the tree, and starts without a screen when there is no graphics output.

**New in the gate:** the group `aarch64: the board from a device tree, under U-Boot` (`normal,smp` on `virt,acpi=off,gic-version=2`, four Cortex-A53).
- It fetches U-Boot once through `scripts/fetch_uboot.sh`, without root: Ubuntu's `u-boot-qemu` 2025.10, pinned by SHA-256, into `~/.cache/mind-u-boot`.
- acc-1, acc-2, PCU and x32-128 all reach both of its sources (a HEAD request each, 2026-11-08).
- The `devicetree` suite (in "programs, shell and four CPUs") now boots to the shell from the tree.

**Tested** in the kernel session's container (Ubuntu 24.04.5, QEMU 8.2.2, AAVMF 2024.02, U-Boot 2025.10, TCG):
- the host tests `fdt` and `fadt`;
- the `devicetree` suite;
- `normal` and `smp` under U-Boot;
- `net` there up to its SFTP login, which needs RNDR (absent on a Cortex-A53, hence not in the group);
- a boot without a display device;
- an ACPI boot under AAVMF;
- x86's `normal`.

The full gate has not run on it yet. That is the round's.
