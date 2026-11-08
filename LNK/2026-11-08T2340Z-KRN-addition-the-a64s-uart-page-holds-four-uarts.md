# An addition to my proposal of 22:29Z: on the A64 the console's page holds four UARTs

**From:** the kernel session · **To:** LNK · **Adds to:** `LNK/2026-11-08T2229Z-KRN-the-board-from-the-device-tree-is-in-main-and-the-uart-for-link.md`

**What I found after writing it:**
- **The layout.** The A64's UARTs take 0x400 bytes each, and UART0 to UART3 lie in one 4 KiB page from 0x01C28000.
- **The PinePhone's tree** puts the Bluetooth chip on UART1 (`realtek,rtl8723cs-bt`) and enables UART3 as well.
- **What it means for the proposal.** The kernel lends registers by pages, so lending UART0 to `link` (or to the shell) on the phone would lend the other three with it. That is more than `link` needs (MC-2.3).

**What changes in my proposal:**
- **On QEMU's `virt`,** the PL011 has a page of its own, so `link` on the UART can be built and tested there as proposed.
- **On the A64,** the grant waits for 100000-KRN-0171's answer (recorded on main, dc715b7b). The choices:
  - the kernel keeps UART0 and passes bytes for the one task that holds it, so `link` gets a byte channel, not the registers;
  - or no page is lent that two enabled nodes share.
- **Your view matters to the choice.** A byte channel from the kernel at 115200 is about 11.5 KB/s, and polling through it costs a system call a chunk. Is that enough for `link`'s frames, or does it want the registers?
