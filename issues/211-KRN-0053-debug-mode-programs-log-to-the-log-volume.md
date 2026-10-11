# 211-KRN-0053 — Debug mode: what programs print goes to the boot log on the log volume

**Type:** kernel (`loader`, `libmind`) · **Owner:** `KRN` · **Priority:** P1 · **Status:** in progress (made; the QEMU check and the Mac's run left) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-12.1, MC-3.11

## Problem

On the MacBook Pro (2026-10-09) the camera and every program that listens failed, and the boot log said nothing about them. The services' lines reach the system log and `log:bootNNNN.log`, but a program's do not: only a process with a log client in `SLOT_LOG` sends its lines to `logd`, and programs get one only when they ask for the log (`REQUEST_LOG`). The maintainer asked for logs and a debug mode in which programs write what they do to the log disk.

## Plan

- **The flag** is a file, `log:debug.txt`. It can be made from the shell (`write log:debug.txt on`) or on any computer that mounts the MIND LOG partition, so it can be set before the machine boots. Removing it ends debug mode.
- **`loader`.** At every launch it looks for the file. While it is there, a program whose launcher lent no log client gets a copy of the loader's own (write only, no read badge). `logd` stamps each line with the program's PID and name. The loader logs when debug mode turns on or off.
- **`libmind`.**
  - `mind::debug::on()` tells a program it runs in debug mode, so it may say more; `mind::debug::check()` asks anew.
  - Lines with escape sequences (a text program drawing its screen) are not sent to the log.
- **Programs.** Their ordinary output is enough at first. `audio_gw` logs every capture (551-DRV-0010) in any mode. `APP` may add `mind::debug::on()` detail to its programs.

## Acceptance criteria

- **QEMU:** with `log:debug.txt`, a program's printed line is in this boot's log file; without it, it is not. A program that asked for the log keeps the shell's client.
- **The MacBook Pro:** with the file on the log partition, the camera and listening programs' lines are in `LOG:bootNNNN.log`.

## Related

[211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md) (the journal), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md).
