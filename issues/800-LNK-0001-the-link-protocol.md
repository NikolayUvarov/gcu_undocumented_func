# 800-LNK-0001 — The link protocol: framed, checked, master and slave, for half-duplex lines

**Type:** protocol and library · **Owner:** `LNK` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-2.4, MC-11.4, MC-12.1

## Problem

The link's ends must agree on one protocol over any byte stream: an RS-485 line, a CDC-ACM port, a vendor bulk pipe. An RS-485 line is half-duplex, so the two ends must never send at once, and noise must not corrupt a file unseen.

## Plan

- **`linkproto/`, a `no_std` Rust crate with no dependency.** MIND Core's end and the host tool share it. `docs/link/protocol.md` describes it in words, for a third implementation.
- **Frames:**
  - COBS-encoded, separated by a zero byte;
  - a header: version 1, channel, type, sequence, acknowledgement, length;
  - a payload of up to 1 KiB, sized down for slow lines;
  - CRC-32C.

  A bad frame is dropped and sent again. A decoder checks every length before it reads (MC-2.4).
- **Master and slave.**
  - The master starts every exchange. The slave answers within a deadline and never sends unasked, so a half-duplex line never has two senders.
  - What the slave has to say (console output, log lines) waits for the master's next poll.
  - On full-duplex transports the rule stays, for simplicity.
- **Channels:**
  - 0, control: hello, versions, roles, capabilities, the frame size;
  - 1, console: bytes both ways;
  - 2, files: put, get, list and stat, by offset, so an interrupted transfer resumes. A whole file is checked by SHA-256 at the end;
  - 3, log: the system log as it is written;
  - 4, input: keyboard and mouse events, for the multi-device to play.
- **Reliability:** each frame acknowledged before the next (stop and wait), retried a bounded number of times, then the exchange fails with its reason.
- **Host tests:** frames with noise, truncation, duplication and loss; a transfer through a lossy fake line; the decoder fuzzed like the IDL decoders (500-ASR-0001).

## Acceptance criteria

- **The crate's host tests pass**, including a 1 MiB file through a fake line that drops, flips and repeats bytes, with its SHA-256 checked at the end.
- **The fuzzer** finds no panic in 100 000 inputs.
- **`docs/link/protocol.md`** exists in English and Russian.

## Related

[800](800-mind-link.md), [800-LNK-0003](800-LNK-0003-the-link-service-on-mind-core.md), [800-LNK-0004](800-LNK-0004-mindlink-the-host-tool.md).
