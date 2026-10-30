# 000-KRN-0188 — Characters typed on the serial line lost under load

**Type:** kernel track (the serial console's input path, the QEMU harness) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.2

## Problem

- **What was seen.** On 2026-10-30, in a trial of two gate lanes on `i9` (`main` 15597f46), `x86: RAM above 4 GiB` failed on the second lane after 1189 s.
  - The harness typed `webcheck https://10.0.2.2:34073/pinned --pin leaf:<64 hex digits> --name other.test`.
  - The guest's echo held 60 of the 64 digits. `webcheck` answered with its usage, and the harness, waiting for its own line's echo, timed out.
  - The other lane was running QEMU suites on the same 16 cores meanwhile.
- **Perhaps seen before.** In round 49's first try, the same group on `un-rizen` ran out its 3500 s with no result, while the second lane ran beside it. Its logs were lost, so this is not known.
- **What the harness does.** `VM.send` writes one byte every 10 ms to QEMU's stdin, which feeds the emulated 16550. QEMU takes a byte only when the UART has room for it, so the loss is likely in the guest, between the UART and the shell's line.

## Plan

- Find where a byte can be dropped between the 16550 and the shell's line under load:
  - the UART's FIFO and its interrupt;
  - the kernel's console ring;
  - the shell's read.
- Count the drops where they happen and log them, then make the path not drop: back-pressure, or a ring that holds a line.
- Reproduce on purpose with four busy loops on the host and a long typed line, before and after the fix.

## Acceptance criteria

- A 200-character line typed 50 times under four busy loops on the host arrives whole every time (QEMU, x86 and aarch64), or each drop is logged where it happens.
- The cause recorded here.

## Related

000-KRN-0186 (the same lanes' load); [docs/effector.md](../docs/effector.md) (the gate's lanes).
