# 100004-PRT-0015 — The Allwinner SoCs' watchdog: a frozen board restarts by itself

**Type:** port (`kernel/src/arch/aarch64`, `common/fdt.rs`) · **Owner:** `PRT` (the kernel session) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (the boards' runs: MOB, 100004-MOB-0002, 0003) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md) · **Roadmap:** track H · **Constitution:** MC-1.2, MC-5.1, MC-12.1, MC-12.2

## Problem

MOB asks for it (`internal-dev-messages`, `KRN/2026-11-12…-MOB-…-watchdog-a-frozen-board-restarts-by-itself.md`). The maintainer wants the SoCs' hardware watchdogs used (2026-11-12).

- **x86 has one.** The chipset's watchdog is the kernel's own: Intel's TCO ([000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md)) and the AMD FCH's ([213-KRN-0125](213-KRN-0125-the-amd-fch-watchdog.md)). It is armed with the tick and fed from the boot CPU's tick.
- **aarch64 has none.** A frozen board stays frozen until someone pulls its power.

The two boards' trees (100004-PRT-0013) name the watchdog by `compatible`:

| SoC | Board | Node | `compatible` |
|---|---|---|---|
| H5 | Orange Pi Zero Plus2 | `watchdog@1c20ca0` | `allwinner,sun6i-a31-wdt` |
| H618 | Orange Pi Zero 3 | `watchdog@30090a0` | `allwinner,sun50i-h616-wdt`, `allwinner,sun6i-a31-wdt` |

The A64 (the PinePhone) names the A31's timer as well.

## Plan

1. **`common/fdt.rs`:** the board's record gets the first enabled node compatible with `allwinner,sun6i-a31-wdt`, by its first `reg`.
2. **`kernel/src/arch/aarch64/watchdog.rs`.** The A31's watchdog registers, as the SoCs' user manuals name them: `CTRL` 0x10 (restart: key 0xA57 and bit 0), `CFG` 0x14 (bits 1:0, 01 a reset of the whole system), `MODE` 0x18 (bit 0 enable, bits 7:4 the interval, 0xB 16 s). Linux's `sunxi_wdt.c` is a cross-check, not a source.
   - **Armed with the tick:** set to reset the whole system 16 s after the last feed, its longest interval (x86's is 30 s); the boot line says so.
   - **Fed** every second from the boot CPU's tick.
   - **Stopped** before a halt the kernel chose, a reboot and a power-off. TF-A's `SYSTEM_RESET` may use the same timer on sunxi, so the kernel does not stand in its way.
   - **Its page is the kernel's:** no task is lent it (`kernel_owns`).
   - **The registers are kept apart** behind a trait, as `fch.rs` keeps the AMD FCH's.
3. **The kernel's budget** (MC-1.2): raised in a commit of its own if the change needs it, naming the mechanism. A watchdog fed from the kernel's own tick is the kernel's: a frozen kernel cannot ask a task to feed it.

## Acceptance criteria

- **Host:**
  - against a register model: armed (configured, then enabled, then fed), fed, stopped; a register that does not take the value refused and left stopped (`tests/sunxi_watchdog_host.rs`);
  - the H5's and the H618's trees give the watchdog at 0x01C20CA0 and 0x030090A0; `virt`'s gives none (`tests/fdt_host.rs`).
- **QEMU `virt`:** no watchdog, and the boot line says so; the aarch64 suites pass as before.
- **The boards (MOB):** on each board, a freeze followed by a reset within the interval, recorded for that board (MC-12.1).

Not covered: a board stopped in U-Boot, a card that does not boot, a wedged chip. For those, the bench's power device (100004-MOB-0009).

## Related

- [100004](100004-two-allwinner-boards-before-the-phone.md): the two boards.
- [000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md), [213-KRN-0125](213-KRN-0125-the-amd-fch-watchdog.md): x86's watchdogs.
- [100004-PRT-0014](100004-PRT-0014-a-kernel-entered-at-el2-goes-on-at-el1.md): the EL2 exit the boards' cards carry.
