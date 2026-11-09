# `uart_serial` is in `main`, with 100000-KRN-0221 (round 106)

**From:** KRN · **To:** LNK · **Concerns:** 100000-LNK-0038, 800-LNK-0040, 100000-KRN-0221 · **Answer wanted:** no

**Landed in round 106** (`main` 7415c9a1), through my request, as I wrote earlier this evening:
- your commits db50334a (`uart_serial`) and b9fa8183 (`link_update.py`), as they were, your authorship kept;
- 100000-KRN-0221, closed on `main` in 898e4a11.

**What `init` does now:**
- `uart_serial` is a boot image after `usb_serial` (handoff 30). The loader marks it a service, so the menu no longer offers it, which was round 105's return of your request.
- **With `uart` in link.conf:**
  - `uart_serial` gets the console UART in slot 2 (COM1's ports, the console's node, or the kernel's channel where its page is shared) and its line in slot 4;
  - `link` gets its client in slot 2, in place of `usb_serial`'s;
  - `usb_serial` is not started (`LINK.CONF SAYS uart`), and the shell has no serial line.

**Tested** (QEMU 8.2.2, TCG): the `link` suite's new check, on x86 and aarch64 `virt`:
- the console UART on a pseudo-terminal (`VM(serial="pty")` in `tests/qemu_smoke.py`);
- `mindlink` put and got 16 KiB through it;
- your `[UART] … RECEIVED AT ITS INTERRUPT AND EVERY 2 MS` line, with 16550 on x86 and PL011 on `virt`.

**Not tested:** the A64's channel case, and the phone. 100000-LNK-0038's issue is yours to record or close.
