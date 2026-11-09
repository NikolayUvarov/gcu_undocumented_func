# KRN ready: the kernel's console as a byte channel (100000-KRN-0222, ABI 29)

**Ready:** `claude/youthful-mendel-mf1soy` 97e8ff15

**From:** the kernel session (KRN, PRT)

**It is** `main` 9c79dcc3 with 100000-KRN-0222's commits on it.

**What it brings:**
- **Where it applies:** where the console's node cannot be lent by its page (the A64's four UARTs share one; a 16550 console). There the shell's serial line is the kernel's channel.
- **ABI 29:**
  - `PLATFORM_CONSOLE` gives a `CAP_KIND_CONSOLE` capability;
  - `TEXT_READ` with it in `msg[0]` takes what the console received (`TEXT_NOWAIT`: no wait);
  - `TEXT_WRITE` with it queues bytes, which the kernel sends from the transmit interrupt.
- **While it stands:** the kernel keeps the console's line, and its log stops going there until no live task holds the channel. x86 refuses it, as COM1 is lent by its ports.
- **The user side:** `init` asks for it; `mind::dev::Uart` drives it, so the shell's code is unchanged.
- **The ring:** `common/text_ring.rs`, the text input's ring, out of `scheduler.rs`, with a host test.
- **For every track:** ABI 29 means every program is rebuilt with it, as with each version. No program's code changes: `mind::io` and `mind::dev` pass `msg` zeroed as before.
- **The kernel's size:** 7394 lines of 7400.

**Tested** in the kernel session's container (QEMU 8.2.2, TCG):
- **aarch64 `devicetree` suite** with the new check, which passed:
  - virt's own tree with a second `arm,pl011` node in the console's page (`-dtb`);
  - the console's node lent to none, and the shell given the channel;
  - every command through it: a 219-byte line typed at once, and 25 reads of 201 bytes.
- **Found and fixed on the way:** the PL011's pending receive interrupt was cleared by each send (the receiver stalled after 16 bytes).
- **The host tests** `text_ring` and `runtime` (ABI 29).
- **Builds:** the kernel, `init` and the shell on both processors.

The full gate is round 101's, which also runs every other group on ABI 29.
