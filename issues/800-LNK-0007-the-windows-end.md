# 800-LNK-0007 — The Windows end of the link

**Type:** host tool · **Owner:** `LNK` · **Priority:** P2 (the maintainer, 2026-10-20: start with the exe) · **Status:** in progress · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-12.1

## Problem

The maintainer also wants Linux to Windows, and MIND Core to Windows. `mindlink` reaches serial ports through `termios`, which Windows lacks.

## Plan

`mindlink` built for Windows reaches the converters through COM ports (`CreateFile`, `SetCommState`). The multi-device's ports are CDC-ACM (800-LNK-0005, changed from a WinUSB channel), so they are COM ports too. `serve` there lets another machine drive it.

## Acceptance criteria

- **Linux to Windows and MIND Core to Windows:** a file each way, and the console of `mindlink serve`.
- **Through the multi-device:** the Windows machine's pointer driven from the other end.

## Progress

**2026-10-20, on `claude/LNK-link`.** The maintainer asked to start with the exe.
- **`mindlink.exe`** is cross-built from the same source (`tools/mindlink/build.sh`, target `x86_64-pc-windows-gnu`) and needs no DLL beyond Windows' own. `port_windows.rs` sets a COM port through `GetCommState`/`SetCommState` (8N1, binary, DTR and RTS on, no flow control) and `SetCommTimeouts`: a read returns at the first byte or after its wait. TCP and `listen:` work as on Linux; `serve`'s console runs through `cmd /C`.
- **Run on the maintainer's PC** through WSL's interop, both ends in Windows over TCP: a 2 MB put and get with matching SHA-256, `ls`, `log`, `stat`.
- **The COM ports** COM4 (FTDI) and COM10 (CH340) open and take their settings. Each is wired to another device, which sends a status line once a second at 9600 bit/s (seen with the new `sniff` command), not to the other converter. So no transfer between them has been tried.
- **2026-10-20, over RS-485:** a second pair of converters (CH340, COM3 and COM22), wired to each other. Between two `mindlink.exe` processes, 10 MiB went each way at 460800 bit/s, 38.1 KB/s, with matching SHA-256. The rates and their limit are in 800-LNK-0004.
- **Left:**
  - a file each way between Windows and WSL, and between Windows and MIND Core;
  - the multi-device's pointer driven from Windows.

## Related

[800](800-mind-link.md), [800-LNK-0004](800-LNK-0004-mindlink-the-host-tool.md).
