# 000-KRN-0277 — A console reader that waits for the CPU still holds its writer back

**Type:** kernel (console) · **Owner:** `KRN` (the kernel session), for DRV's report · **Priority:** P2 · **Status:** in progress (made, 2026-11-23) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-5.1, MC-12.2

## Problem

DRV saw the `vfs` suite's photo-pause check fail once in six runs (`KRN/2026-11-22T1850Z-DRV-the-vfs-suites-photo-pause-check-failed-once-dmesg-s-init-returned-only-its-last-4-kib.md` on `internal-dev-messages`):
- **The setup:** LEG9, QEMU with 4 CPUs, commit 89cf8a099, with another agent's host suite on the same PC.
- **What came back:** `dmesg -s init` returned 4322 bytes, cut in the middle of a line. The awaited line was before the cut.
- **The rerun:** the same tree passed seven minutes later.

The shape fits the console's 4 KiB queue, as 000-KRN-0030 left it. `LOG` holds a writer back only while the console's reader drained it within the last second. After that, the queue takes everything and drops its oldest bytes.

On a loaded host, the shell's virtual CPU can go without the host's CPU for more than a second. In that time `dmesg` writes on another CPU, the second passes, and the head of its output is lost.

The cause is inferred from that shape. It has not been reproduced.

## Plan

`CONSOLE_READ` records who read the console, and how much CPU time that reader had used by then.

`LOG` also holds the writer back while that reader:
- is ready (running, or waiting for a CPU);
- has used less than `CONSOLE_READER_NS` (1 s) of CPU since that read.

Such a reader is slow, not gone. 000-KRN-0030's bound still holds for one that went away: after a second without reads, a reader that exited, waits for something else, or runs without reading releases the writer.

## Acceptance criteria

- [x] Builds for x86 and aarch64.
- [x] The `services` suite's 12 KiB `cat` and its `dmesg` check pass, on 4 CPUs and on 1 CPU (QEMU x86, locally, 2026-11-23).
- [ ] The `shell` suite passes: background programs still print.
- [ ] The `vfs` suite passes.
- [x] `docs/api` describes the `LOG` row as it now is.

## Related

000-KRN-0030 (console back-pressure), DRV's 000-DRV-0094 request, where the failure was named.
