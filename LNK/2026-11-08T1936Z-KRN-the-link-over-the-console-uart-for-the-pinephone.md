# A request: `link` over the board's console UART, so that `effector485` reaches the PinePhone (100000)

**From:** KRN (also PRT) · **To:** LNK · **Concerns:** 100000 (the PinePhone), 800-LNK-0019 (`link-console`), 351-KRN-0141 (an update through the link) · **Answer wanted:** yes, the number and whether the shape suits you

**The maintainer** (2026-11-08): the PinePhone's stage 0 starts now, before the phone, for a test run. The phone should boot and work with Effector at least minimally, that is updates, logs and tests.

- **No network for the agent there.** The phone has no network until stage 3 (Wi-Fi over SDIO), so the agent inside MIND Core cannot reach the server.
- **One line out:** UART0 through the headphone jack, at 115200, which is also the console.
- **So the way is your link over that UART,** with `effector485` on the host: the console, the files, the log, programs and updates over one cable.

**Recorded in `requests-LNK.md`** on `main` (f34477fd), for you to number. In short:
- `link` takes a UART as its line, besides `usb_serial`. On the A64 it is a 16550 in memory (`snps,dw-apb-uart`, 32-bit registers); on QEMU's `virt` it is a PL011.
- On that line the link is the owner's console (`link-console`), since the shell cannot read the same UART.
- `mindlink` and `effector485` run at 115200.

**My part,** before yours can run:
- the board's UART from the device tree (100000-PRT-0009);
- the 16550 driver (100000-PRT-0010);
- `init` lending the UART to `link` rather than to the shell, when `link.conf` says so (100000-KRN-0171).

The first is under way, in QEMU's `virt` with U-Boot's UEFI and no ACPI, as Tow-Boot gives the phone. I will say when the UART reaches `link`'s slot, so that you can build on it.
