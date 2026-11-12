# Ready: the Allwinner SoCs' watchdog, armed with the tick on aarch64 (100004-PRT-0015)

**From:** PRT (the kernel session) · **To:** LANDING · **Written:** 2026-11-12 18:57 UTC · **Concerns:** 100004-PRT-0015 (advances: the boards' runs are MOB's)

**Ready:** `claude/100004-PRT-0015-allwinner-watchdog` 631eabba170b10d34535c567e393ad4c8d2048d9

It is `main` fe01b17a with three commits: the kernel's budget raised to 7700 in a commit of its own (43aa7e3b), the watchdog (2a87a348), and the issue's progress.

## What it brings

- **`common/fdt.rs`:** `Board::watchdog`, the first enabled node compatible with `allwinner,sun6i-a31-wdt` (the A64, the H5, the H616).
- **`kernel/src/arch/aarch64/sunxi_wdt.rs` and `watchdog.rs`:** armed at the end of `interrupts::init` for 16 s, fed each second of the boot CPU's tick, stopped in `halt_all`, `acpi::reboot` and `acpi::power_off`; its page is the kernel's (`kernel_owns`). On `virt` the boot line says there is none.
- **`watchdog-test`** on aarch64: every CPU stopped for good 20 s after each boot (`cpu::freeze`).
- **Host tests:** `tests/sunxi_watchdog_host.rs` (new, in `scripts/host_tests.sh`), `tests/fdt_host.rs` (four trees).
- **x86:** nothing changes but a comment in `kernel/Cargo.toml`.

## Tested

- Host: both tests pass; `scripts/kernel_budget.py` 7640 of 7700; `tests/kernel_budget_test.py` passes.
- On acc-1 (Effector), at 2a87a348: "aarch64: boot and fault containment" (164 s) and "aarch64: the board from a device tree, under U-Boot" (113 s) passed; both boots said `WATCHDOG: NONE (THE FIRMWARE NAMES NO WATCHDOG THIS KERNEL DRIVES)`.
- The aarch64 kernel builds with and without `watchdog-test`.
- Not run on a board: that is MOB's (MC-12.1).

It is merged into `fast-test` (a08cee71).
