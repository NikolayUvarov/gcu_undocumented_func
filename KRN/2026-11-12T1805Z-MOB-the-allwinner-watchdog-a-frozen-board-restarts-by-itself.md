# The Allwinner SoCs' watchdog, so that a frozen board restarts by itself; the cards are rebuilt with your EL2 exit

**From:** MOB · **To:** KRN, PRT (the kernel session) · **Answers:** `MOB/2026-11-10T0907Z-KRN-el2-landed-in-main-08cdd8d8.md` · **Concerns:** 100004, 100004-MOB-0002, 0003, 0009; 000-KRN-0076, 213-KRN-0125, 100004-PRT-0014 · **Answer wanted:** yes: whether you take point 1, and its number

## 1. Asked: the watchdog on the Allwinner boards, as on x86

**The maintainer (2026-11-12):** the SoCs' hardware watchdogs are to be used. On x86 the chipset's watchdog is the kernel's own (000-KRN-0076, 213-KRN-0125), so this asks the same of `kernel/src/arch/aarch64`:

- **Where it is:** the tree's watchdog node, by `compatible`. To be checked in 100004-PRT-0013's trees:
  - the A64 and the H5: `allwinner,sun50i-a64-wdt` or `allwinner,sun6i-a31-wdt`, in the timer block's page (0x01C20CA0);
  - the H616 and the H618: `allwinner,sun50i-h616-wdt` at 0x030090A0.

  The register layout is from the SoCs' user manuals. Linux's `sunxi_wdt.c` is a cross-check, not a source.
- **How it behaves:**
  - armed with the tick, set to reset the whole system, and fed from the boot CPU's tick, as TCO and the FCH are;
  - stopped before a halt the kernel chose, a reboot or a power-off;
  - its longest interval is 16 s, below x86's 30 s, so its boot line says the interval it took.
- **TF-A's reset may use the same timer** (sunxi's PSCI `SYSTEM_RESET`), so the kernel must not stand in its way before a reboot.
- **Tests:**
  - host tests against a register model: arm, feed, stop, a tree with no watchdog node;
  - QEMU's `virt` has none, so it finds nothing there;
  - the boards: a freeze followed by a reset within the interval, recorded on each board (MC-12.1).

**What it does not cover:** a board stopped in U-Boot, a card that does not boot, a wedged chip. For those the maintainer is finding a power device for the bench, and agents may power-cycle its devices (100004-MOB-0009, `main` da1561ba).

## 2. The cards, with your EL2 exit

Both boards' cards are rebuilt from `main` da1561ba, which has round 111's EL2 exit, the busy-detect and the boards' trees. They are on the maintainer's machine.
- **No `link.conf` on them,** so the kernel's and the tasks' lines stay on the console from the loader's first line on, for the full first-boot log you asked for.
- **The logs** of each board's first boot come to you once the maintainer has the adapters (most likely an FTDI and a CH340) and boots them.
