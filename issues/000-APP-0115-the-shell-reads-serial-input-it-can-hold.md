# 000-APP-0115 — The shell reads only as much serial input as one pass of its loop holds

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P1 (a gate's failure, 000-KRN-0188) · **Status:** in progress (done in QEMU on x86; KRN's check under load left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-12.1

Numbered on 2026-10-30 from the kernel track's request in `requests-APP.md`, "The shell drops serial input past 64 events in one pass of its loop", for [000-KRN-0188](000-KRN-0188-typed-characters-lost-under-load.md).

## Problem

- **The loop:** it read the UART while it had a byte (`while let Some(byte) = … Uart::read`), decoding into `Events`, a list of 64.
- **What was lost:** `Events::push` dropped what did not fit. Its comment assumed the 16550's FIFO bounds a pass at 16 bytes, but QEMU refills the FIFO at each read.
- **What KRN showed:** a line of 190 characters written at once kept 64 bytes, and its newline was lost. Under host load the harness's one byte per 10 ms does the same once the shell is held for about 0.65 s. That fits `RAM above 4 GiB`'s 4 lost hex digits of 2026-10-30.

## Plan

- **The loop** reads while `Events` has room for three more events: a byte gives at most two, and `vt.poll` one more. The rest stays in the UART for the next pass, behind QEMU's flow control or a FIFO.
- `Events::room`, and the comment on `Events` corrected.

## Acceptance criteria

- **The `shell` suite:** `logger` and 193 characters, written to the serial line at once, arrive whole, and `dmesg -s shell` holds them.
- **KRN's criterion in 000-KRN-0188**, which KRN adds to the `shell` suite: a line of 200 characters typed 50 times under four busy loops on the host arrives whole every time, on x86 and aarch64.

## Related

[000-KRN-0188](000-KRN-0188-typed-characters-lost-under-load.md).
