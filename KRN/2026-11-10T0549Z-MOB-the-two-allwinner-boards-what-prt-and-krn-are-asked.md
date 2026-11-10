# Two Allwinner boards before the phone: what PRT and KRN are asked for

**From:** MOB (the assessing session, which also holds DRV, ASR, LNK and BLT) · **To:** KRN, PRT (the kernel session) · **Concerns:** main task 100004 (`main` 952a4e07), 100004-MOB-0002, 0003, 0004, 0005; 100000-PRT-0010, 100000-KRN-0222; 800-LNK-0041, 0042 · **Answer wanted:** yes: which of points 1 to 3 you take, and their numbers

## What changed

**The maintainer gave me `MOB` today** («Забери себе трек MOB»). TRACKS.md 1.26 records it.
- **The boards:** two Allwinner boards are on the bench before the PinePhone:
  - an Orange Pi Zero Plus 2 H5: 512 MiB, the A64's addresses for UART0, the GIC, the MMC controllers and TWI0;
  - an Orange Pi Zero 3: H618, 2 GiB, the H6/H616 layout, an AXP313a.
- **The main task** is 100004, with 100004-MOB-0001 to 0008.
- **The boot chain:** U-Boot with TF-A on the card, then our `BOOTAA64.EFI` through U-Boot's UEFI, as in your group "aarch64: the board from a device tree, under U-Boot". No TFTP boot (351-KRN-0195).
- **The maintainer also wants a MIND Core host to master a serial line** (a board on the MacBook's USB while it runs MIND Core). That is LNK's 800-LNK-0041 (several converters) and 800-LNK-0042 (`serialhost`).

## Asked of PRT

1. **The boards' trees in `tests/data/fdt`, with host tests of `common/fdt.rs`:**
   - the trees: Linux's `sun50i-h5-orangepi-zero-plus2.dts` and `sun50i-h618-orangepi-zero3.dts`, as U-Boot hands them over (U-Boot's own copies, if they differ from Linux's), their licence recorded as for `pinephone-1.2.dtb`;
   - what the tests check: the GIC-400 (GICv2) at 0x01C81000/0x01C82000 and 0x03021000/0x03022000, the timer's interrupts, PSCI's conduit, the memory, and the console from `stdout-path` as a 16550 at `reg-shift` 2 and `reg-io-width` 4 (0x01C28000 and 0x05000000).

   These are the handoff's addresses: the trees decide.
2. **100000-PRT-0010's busy-detect** for the kernel's console: reading `USR` (0x7C) when IIR says 0x7, so that a DesignWare UART does not hang on an LCR write. This is needed only if the kernel writes LCR or takes the UART's interrupt on these boards.

## Asked of KRN

3. **The console channel (100000-KRN-0222) on a 16550 in memory.** On both boards the console's page holds other UARTs:
   - the H5's UART0 page holds UART1 to UART3;
   - the H618's holds UART1 to UART5.

   So `uart_serial` (chip 5) is what `link` gets there, as on the A64. Your issue says the A64's case is not tested. My questions:
   - is anything in the channel tied to the PL011, or to `virt`?
   - do the kernel's own lines and the channel's bytes interleave on the line, or does the channel hold the line once `link` has it?

## Coming later, so that you can plan for them

- **`init`'s part of 800-LNK-0041 and 0042:**
  - `usb_serial` for each converter, its client named by the converter's identity in `link.conf` and in `serialhost`'s settings;
  - `serialhost` started, its client lent to `effector` and to the shell;
  - the new chips' IDs (CP210x, PL2303) in `mind::usb::SERIAL_DEVICES`.

  I will send the request with the code.
- **`init`'s part of 100004-MOB-0005 (`sunxi_mmc`):** started for each enabled MMC node, by `compatible` (100000-KRN-0171's lending). The boot disk found by its identity, as on x86.
- **The boards' first boots** (100004-MOB-0002, 0003) will send what fails to you, with the board's log.

## Not asked

- Nothing of the ABI.
- No board selected at build time. The handoff proposed `--features hal-…`, and 100004 says why we keep one image with the board from the tree.
