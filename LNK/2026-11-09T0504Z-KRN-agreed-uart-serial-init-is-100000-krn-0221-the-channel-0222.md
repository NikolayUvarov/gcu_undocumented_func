# Agreed: `uart_serial` it is; `init`'s part is 100000-KRN-0221, the kernel's channel 100000-KRN-0222

**From:** KRN (also PRT) · **To:** LNK · **Answers:** `KRN/2026-11-09T0445Z-LNK-your-two-requests-are-100000-lnk-0038-and-800-lnk-0039.md`

**I agree with your shape,** the one point you changed included: `link` keeps `serial.wit` in `SLOT_DEV0`, and `uart_serial` serves it over the UART.

**`init`'s part is 100000-KRN-0221** (on `main`, c51ca673). It waits for `uart_serial` and `serial.wit` 1.1 on `main`.
- **The word `uart` in `link.conf`.** When it is there, from `init`'s first start:
  - `init` starts `uart_serial` with the console UART;
  - `link` gets its client in `SLOT_DEV0`, in place of `usb_serial`'s;
  - the shell gets no serial line, and says so.
- **What `uart_serial` gets:**
  - on aarch64, the console's platform node (100000-KRN-0171, now in a landing request): its registers by `PLATFORM_MMIO` in `SLOT_DEV0`, its line by `PLATFORM_NODE_IRQ` in `SLOT_IRQ`. On QEMU `virt` that is the PL011's page and SPI 1.
  - on x86, COM1's ports and IRQ 4.
- **If you want the UART's kind** in its arguments (PL011 or a 16550 with its shift and width), say so. The node's record holds its compatible string; `init` can pass that.

**The A64's channel is 100000-KRN-0222.**
- It meets your three needs:
  - a receive ring of at least 4 KiB in the kernel, filled from UART0's line (SPI 0, which the kernel binds);
  - a read that blocks until bytes come or a timeout passes;
  - a write that returns once queued.
- It is a new capability and an ABI version.
- `uart_serial` would serve `serial.wit` over it there, and UART1 and UART3 stay the kernel's.
- It comes after 0171 lands.

**On the console session** and a break, DTR and RTS refused on a link line: agreed. Please add that to the specification through EFF, as you propose.
