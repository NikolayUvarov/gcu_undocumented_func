# 800-LNK-0008 — MIND Core as the master: a shell program drives the link to another machine

**Type:** service and program · **Owner:** `LNK` · **Priority:** P2 · **Status:** open · **Blocked by:** [800-LNK-0003](800-LNK-0003-the-link-service-on-mind-core.md) (`init`'s part, KRN) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-10.2, MC-12.1

## Problem

The maintainer asked for both ends, master and slave, on Linux and on MIND Core (2026-10-20). That covers Linux to MIND Core, Linux to Windows, and MIND Core to Linux, Windows or MIND Core.

Today MIND Core is only a slave: its `link` service answers another machine's `mindlink`. A MIND Core machine cannot fetch a file from another machine, put one there, or read its log.

## Plan

- **`link` in either role.** `link.conf` names the role (`role=slave`, the default, or `role=master`). In the master role, `link` serves a request interface (`idl/link.wit` 1.0) on its service endpoint instead of answering the line. It is still the only holder of `usb_serial`'s client (MC-2.3).
- **The requests:** hello, put, get, ls, stat, rm, log of the other end. Files come from and go to `data/link/` only, as in the slave role.
- **A shell program, `mlink`,** with the commands of `mindlink` (800-LNK-0004): `mlink put <file>`, `mlink get <name>`, `mlink ls`, `mlink log`. The shell lends it `link`'s client on request.
- **Logged:** each request, file and SHA-256 (MC-10.2).
- **Tested** in QEMU: one MIND Core in the master role, its emulated FT232 on a pseudo-terminal, and `mindlink serve` on the host as the other end. Two MIND Core machines in QEMU, back to back, where QEMU can join their chardevs.

## Acceptance criteria

- **In QEMU, MIND Core's `mlink`** puts and gets 1 MiB with matching SHA-256 against `mindlink serve`, and reads its log.
- **On the maintainer's machines,** the same through the RS-485 converters, MIND Core to Linux.

## Related

[800](800-mind-link.md), [800-LNK-0003](800-LNK-0003-the-link-service-on-mind-core.md), [800-LNK-0004](800-LNK-0004-mindlink-the-host-tool.md), [800-LNK-0007](800-LNK-0007-the-windows-end.md).
