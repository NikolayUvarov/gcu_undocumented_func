# Requests for the MIND Link track (LNK), not numbered yet

**Owner:** MIND Link track · **Status:** open (1 waiting: the kernel session's link over the board's console UART, for the PinePhone, 2026-11-08) · **Recorded by:** the tools track (APP), 2026-10-30

The LNK track numbers its own tasks (`NNN-LNK-MMMM`), so requests from other tracks wait here. It turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## MIND Core's `link` over the board's console UART, for the PinePhone (for 100000, stage 0 and 1)

**Asked by** the kernel session, 2026-11-08, at the maintainer's word: the PinePhone's first test run is to boot and to work with Effector minimally, that is updates, logs and tests, before Wi-Fi exists there.

**Why the link.**
- The phone's only line out is UART0, through the headphone jack, at 115200 8N1. It is also the console.
- The agent inside MIND Core needs a network, and the phone has none until stage 3 (Wi-Fi over SDIO).
- `effector485` over the link gives, over one cable:
  - the console (channel 1, 800-LNK-0019's `link-console`);
  - files and the log;
  - programs started;
  - updates (351-KRN-0141, 351-LNK-0013).

**What it needs of LNK:**
- `link` taking a UART as its line, besides `usb_serial`. The UART is the board's console: a 16550 in memory on the A64 (`snps,dw-apb-uart`, 32-bit registers), a PL011 on QEMU's `virt`.
- On that line the link is the owner's console, since the shell cannot read the same UART. That is `link-console`, with the boot's first lines readable before the link answers.
- `mindlink` and `effector485` at 115200, the rate Tow-Boot leaves the line at, unless the link may change it after the handshake.

**What it needs of KRN** (the kernel session's part, made with it):
- the board's UART from the device tree (100000-PRT-0009);
- the 16550 driver in memory (100000-PRT-0010);
- `init` lending the UART's registers and line to `link` rather than to the shell, when `link.conf` says so (100000-KRN-0171).

**In QEMU first:** `virt` with U-Boot's UEFI and no ACPI (as Tow-Boot gives the phone), its PL011 on a pty, with `mindlink` and `effector485` on the host.

