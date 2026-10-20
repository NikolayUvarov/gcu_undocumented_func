# 800-LNK-0003 — `link`, MIND Core's end of the link: files, the log and a console, under explicit grants

**Type:** service · **Owner:** `LNK` · **Priority:** P1 · **Status:** open · **Blocked by:** [800-LNK-0001](800-LNK-0001-the-link-protocol.md), [800-LNK-0002](800-LNK-0002-usb-serial-ftdi-and-ch340.md); `KRN`: `init` starting `link` with its grants · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-3.11, MC-10.2, MC-11.4

## Problem

A MIND Core machine needs an end of the link that the other machine can reach. That end must not hand the other machine more authority than the maintainer granted: a link that could write anywhere, or run anything, would be a hole.

## Plan

- **`link`, a ring-3 service.** It plays master or slave (800-LNK-0001) over a `serial.wit` port (800-LNK-0002), and later over the multi-device's channels (800-LNK-0006).
- **Its authority is only what `init` grants** (MC-2.3):
  - **files:** a file client confined to `data/link/`, where the other end puts and gets files;
  - **the log:** a reader of the system log;
  - **the console:** only when the maintainer turns it on. The other end then gets a session of the shell, with the authority a session has (APP's part). It is off by default.
- **On request only.** `link` runs when the boot volume holds `link.conf` at its root (the transport, the role, the rate, the console's switch). As with `autorun.msh`, only whoever writes the disk can put it there.
- **Logged.** Every exchange is logged (MC-10.2): the peer's hello, each file and its SHA-256, the console opening and closing.
- **Untrusted input.** Everything that comes over the link is checked by `linkproto`'s decoder before any use (MC-11.4).

## Acceptance criteria

- **In QEMU, through the emulated FT232 and a host socket,** the host tool (800-LNK-0004):
  - puts and gets a file in `data/link/` with matching SHA-256;
  - reads the system log;
  - is refused any path outside `data/link/`.
- **On the MacBook Pro or the Ryzen,** the same through the RS-485 converters, from WSL.

## Related

[800](800-mind-link.md), [800-LNK-0004](800-LNK-0004-mindlink-the-host-tool.md), 000-APP-0079 (the root as the only place for a start file).
