# 800-LNK-0004 — `mindlink`, the Linux and WSL end of the link

**Type:** host tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — ([800-LNK-0001](../issues-done/800-LNK-0001-the-link-protocol.done) done) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-12.1

## Problem

The maintainer develops in WSL and Linux, so the link's other end runs there. It must drive a serial port (`/dev/ttyUSB*`), and later the multi-device's channels, with one command per task.

## Plan

- **`tools/mindlink/`, a Rust program built on the host** with `linkproto` (800-LNK-0001). It uses the standard library only: a serial port through `termios`.
- **Commands:**
  - `mindlink <port> hello`;
  - `mindlink <port> put <file> [<name>]` and `get <name> [<file>]`;
  - `mindlink <port> ls`;
  - `mindlink <port> log [-f]`;
  - `mindlink <port> console`.

  Each prints what it did and the SHA-256 of every file moved.
- **`mindlink <port> serve <dir>`** plays the slave on Linux, so two Linux machines, or one with both converters, can test the line without MIND Core.
- **WSL.** `docs/link/wsl.md` says how a USB converter reaches WSL: `usbipd-win` (`usbipd bind`, `usbipd attach --wsl`), then `/dev/ttyUSB0`. In English and Russian.

## Acceptance criteria

- **On the maintainer's PC,** with both converters wired back to back over RS-485, `mindlink /dev/ttyUSB0 serve` on one and `put` and `get` on the other move a 10 MiB file each way with matching SHA-256. The rate is logged.
- **The same tool** reaches MIND Core's `link` (800-LNK-0003) in QEMU and on a real machine.

## Progress

**2026-10-20, on `claude/LNK-link`.**
- **`tools/mindlink/`** is written and host-tested:
  - every command of the plan, plus `stat` and `rm`. `put` skips a file the other end already has (by SHA-256) and resumes an interrupted one;
  - ports: a serial port (termios, rates 9600 to 3 000 000), `tcp:`, `listen:` and `pty`;
  - `serve <dir>` with `--log <file>` and `--console <command>`;
  - `hello` warns when an FTDI port's latency timer is over 2 ms.
- **The page [docs/link/mindlink.md](../docs/link/mindlink.md)** (Russian [mindlink_RU.md](../docs/link/mindlink_RU.md)) holds the WSL part the plan gave to `docs/link/wsl.md`: usbipd, the drivers, the latency timer and the RS-485 wiring.
- **The tests, `tools/mindlink/tests/mindlink.rs`** (in `scripts/host_tests.sh`): every command against `serve` over a pseudo-terminal, 1.5 MB each way; a put over TCP; a stopped server failing in bounded time. A manual run over a pseudo-terminal moved 3 MB each way at about 1.3 MB/s. That says nothing of a real line.
- **Left:**
  - the back-to-back run of the two converters, which needs usbipd-win installed and both converters attached to WSL. On the maintainer's PC they are COM4 (FTDI) and COM10 (CH340);
  - MIND Core's end (800-LNK-0002, 0003).

## Related

[800](800-mind-link.md), [800-LNK-0003](800-LNK-0003-the-link-service-on-mind-core.md), [800-LNK-0007](800-LNK-0007-the-windows-end.md).
