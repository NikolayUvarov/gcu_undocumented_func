# 800-LNK-0007 — The Windows end of the link

**Type:** host tool · **Owner:** `LNK` · **Priority:** P3 · **Status:** open · **Blocked by:** [800-LNK-0004](800-LNK-0004-mindlink-the-host-tool.md), [800-LNK-0005](800-LNK-0005-the-rp2-multi-device-firmware.md) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-12.1

## Problem

The maintainer also wants Linux to Windows, and MIND Core to Windows. `mindlink` reaches serial ports through `termios`, which Windows lacks.

## Plan

`mindlink` built for Windows reaches the converters through COM ports (`CreateFile`, `SetCommState`) and the multi-device's fast channel through WinUSB. The firmware's Microsoft OS 2.0 descriptors let Windows bind WinUSB with no driver installed (800-LNK-0005). `serve` there lets another machine drive it.

## Acceptance criteria

- **Linux to Windows and MIND Core to Windows:** a file each way, and the console of `mindlink serve`.
- **Through the multi-device:** the Windows machine's pointer driven from the other end.

## Related

[800](800-mind-link.md), [800-LNK-0004](800-LNK-0004-mindlink-the-host-tool.md).
