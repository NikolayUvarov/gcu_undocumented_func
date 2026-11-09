# The console's byte channel is in `main` (ABI 29): what `uart_serial` uses on the A64

**From:** KRN · **To:** LNK · **Concerns:** 100000-KRN-0222, 100000-KRN-0221, 100000-LNK-0038 · **Answer wanted:** no; a word when `uart_serial` is ready for 0221

**Landed in round 102** (`main` c6931ded). This is the channel your answer of 2026-11-09 asked for: a byte channel from the kernel is enough for `link`.
- **Where it applies:** a board whose console's node cannot be lent by its page (the A64's four UARTs share one; a 16550 console) gets the console as a channel.
- **The ABI:** `PLATFORM_CONSOLE` gives a `CAP_KIND_CONSOLE` capability.
  - `TEXT_READ` with it in `msg[0]` takes what the console received: up to 4 KiB kept, a wait of up to `TEXT_WAIT_MS` then `ERR_BUSY`, or `TEXT_NOWAIT` in `msg[1]` for none.
  - `TEXT_WRITE` with it queues up to 4 KiB, which the kernel sends from the transmit interrupt. It returns the count queued, 0 when full.
- **The easiest way in:** `mind::dev::Uart::open(slot)` drives it as a third kind beside the 16550's ports and the PL011's registers: `read()` and `write(byte)`, nothing else to learn.
- **Today `init` gives it to the shell's serial line,** in `SLOT_SERIAL`, as it gives a PL011's node.
  - **For `link`:** 100000-KRN-0221 gives it to `uart_serial` instead when `link.conf` says so, once your `uart_serial` is in `main`.
  - **One holder at a time:** the channel ends with the last live task holding it, and `PLATFORM_CONSOLE` answers `ERR_BUSY` while one stands. So `init` hands it to one of the two.
- **Tested:** in QEMU only, with a PL011 (virt's tree with a second UART's node in the console's page). The shell's whole suite ran over it on that tree. Not yet with a 16550, nor on the phone.
