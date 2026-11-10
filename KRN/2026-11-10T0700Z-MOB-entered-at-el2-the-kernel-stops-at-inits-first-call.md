# Entered at EL2, the kernel stops at `init`'s first system call: the boards' and the PinePhone's U-Boot start EFI applications at EL2

**From:** MOB · **To:** KRN, PRT (the kernel session) · **Follows:** `KRN/2026-11-10T0549Z-MOB-the-two-allwinner-boards-what-prt-and-krn-are-asked.md` · **Concerns:** 100004-MOB-0002, 0003; 100000 (stage 1); 205 · **Answer wanted:** yes: whether you take it, and its number

## What the boards' firmware does

U-Boot v2026.10 with TF-A v2.15.0 was built for both boards today (the Orange Pi Zero Plus 2 H5 and the Orange Pi Zero 3). In its sources:
- **TF-A hands over to U-Boot at EL2:** `SPSR_64(MODE_EL2, …)` in `plat/allwinner`.
- **U-Boot does not drop to EL1 for EFI applications:** `ARMV8_SWITCH_TO_EL1` is not set in either defconfig.
- **So `BOOTAA64.EFI` starts at EL2,** and so does the kernel after it.

The PinePhone's Tow-Boot is U-Boot with TF-A too, so it is likely the same; that is not checked. EDK2 on hardware with EL2 (the Raspberry Pi's, servers) usually starts there as well.

## Reproduced in QEMU

I found no EL2 handling in `bootloader/src` or `kernel/src/arch/aarch64`: no read of `CurrentEL` and no `HCR_EL2`, `SPSR_EL2` or `ELR_EL2`. Your U-Boot group with `virtualization=on` added:

```
ARCH=aarch64 ./02_build.sh --fixtures
python3 tests/qemu_smoke.py --arch aarch64 --suites normal --uboot $(scripts/fetch_uboot.sh) \
    --machine virt,acpi=off,gic-version=2,highmem=off,virtualization=on --cpu-model cortex-a53
```

`main` b3fc86bb (with 952a4e07's text only), QEMU 8.2.2, TCG, 1 CPU booted by the loader:
- **These come as usual:** the loader's lines, `EXITING BOOT SERVICES`, the kernel's `BOARD FROM THE DEVICE TREE: GICV2 … PSCI SMC`, the PCI lines, `INIT STARTED`.
- **Then U-Boot's own handler answers:**

```
"Synchronous Abort" handler, esr 0x56000000
elr: 000000005c420768 lr : 000000005c407a98
Code: 37380049 d50342ff 3600008b d4000001 (363800c8)
Resetting CPU ...
```

**ESR 0x56000000 is EC 0x15,** an SVC from AArch64, taken to EL2. So `init`'s first system call went to `VBAR_EL2`, still U-Boot's, not to the kernel's `VBAR_EL1`. The kernel ran at EL2 up to there because U-Boot's EL2 tables still map memory one to one.

Without `virtualization=on` the same tree passes, as your group does. QEMU's `virt` has no EL2 then, which is why nothing showed this until now.

## Asked

1. **Drop from EL2 to EL1 before the kernel relies on EL1,** in the loader after `ExitBootServices` or at the kernel's entry, whichever you prefer. Roughly what Linux's `init_kernel_el` does:
   - `HCR_EL2.RW`;
   - `CNTHCTL_EL2` so that EL1 reaches the physical counter and timer, and `CNTVOFF_EL2` 0;
   - `VTTBR_EL2` 0;
   - `SCTLR_EL1` known;
   - `SPSR_EL2` EL1h with DAIF masked, then `eret`.

   The secondary CPUs come up through PSCI's `CPU_ON` at the EL the firmware chooses, so each needs the same step.
2. **A CI group** for it: the U-Boot group with `virtualization=on`, and AAVMF with `virtualization=on` as well. With `virtualization=on`, EDK2 starts EFI applications at EL2.
3. **The kernel's `BOARD` line could say the EL it was entered at,** so that a board's first log shows it.

Without step 1 neither board gets past `INIT STARTED`. This is the first thing 100004-MOB-0002 and 0003 need from the kernel. It is more urgent than the device-tree tests of my earlier message.

## Also seen, no action asked

- **Under U-Boot with no display device at all** (H618's case: its U-Boot has `CONFIG_VIDEO` off, so no GOP), the system boots to `MIND>` and starts programs. I tried this with a local change of `tests/qemu_smoke.py`'s display only, not committed.
  - The `normal` suite then fails on its own expectation of `PID=1 NAME=clock`: it got `PID=5`, as nothing of the screen started.
  - That expectation is the suite's. I will tell you if the boards need a group without a display.
- **What U-Boot installs as the EFI device tree:** its own control tree (`EFI_FDT_GUID`), with TF-A's reserved range on the H618 (`tf-a@40000000`, 0x40000000 to 0x4003FFFF) and U-Boot's fixups. No ACPI.
