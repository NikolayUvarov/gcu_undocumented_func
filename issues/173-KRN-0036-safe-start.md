# 173-KRN-0036 — A safe start: the service configuration ignored when a key is held at the bootloader

**Type:** kernel (ABI) and bootloader · **Owner:** `KRN`, with `PRT` (the bootloader's lines) · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [173](173-boot-services-configuration.md) · **Constitution:** MC-6.1, MC-12.4

## Problem

A configuration that disables what a machine needs, such as its network on a machine reached over it, has to be undone without that service. The way back must not depend on the configuration itself.

## Plan

- The bootloader shows `MIND CORE BOOT: HOLD ESC FOR A SAFE START` during its pause and reads the keyboard through the firmware's text input.
- A key held there sets `BOOT_SAFE` in a flags field of `BootInfo`. That is an ABI change: a new version if ABI 4 is on `main` by then.
- With the flag set, init does not read `data/services.txt`, starts every service, and logs `[INIT] SAFE START: THE SERVICE CONFIGURATION IS IGNORED`. `boot-plan` says so.
- **Test:** QEMU sends the key through the monitor at the bootloader's pause. The configuration of 173-KRN-0035's test is then ignored.

## Acceptance criteria

The test passes on x86 and aarch64. Without the key the configuration applies as before.

## Progress

2026-10-24, the kernel track (it holds `PRT` too):

- **The loader** (`bootloader/src/main.rs`, `safe_start`).
  - After the photo pause it prints `MIND CORE BOOT: ESC OR S WITHIN 1 S: A SAFE START, data/services.txt IGNORED`. Then it reads the firmware's text input for a second (`SAFE_KEY_MS`).
  - A key pressed since the loader started counts too. That is how the suite asks: it types S over the serial line once the loader's first line is out, since Esc on a serial terminal waits for an escape sequence.
  - Asked, it drops the keys a held key repeats and prints `SAFE START ASKED: init WILL IGNORE data/services.txt`.
  - The second is added to every boot.
- **The handoff.** `BootInfo.boot_flags` with `BOOT_SAFE` (`common/abi.rs`), at the end of the record. Handoff 20 brings it; the kernel still takes loaders from 16 on. For an older loader the kernel zeroes the field (`BootInfo::clear_unwritten`): it lies in what such a loader left as padding, so the record's size stays 1048 bytes.
  - This is the "minor" step the handoff's rule now names: a field at the end, read only from the version that brought it, raises `BOOT_HANDOFF_VERSION` alone. A loader of handoff 20 is needed only for the key; an older one boots this kernel as before.
- **init.** With the flag set it logs `[INIT] SAFE START ASKED AT THE BOOTLOADER: THE SERVICE CONFIGURATION IS IGNORED (data/services.txt NOT READ)` and does not read the file. It starts every service in its order.
  - `init.wit` 1.10 adds `configuration`: `absent`, `read`, `ignored` or `unreadable`, where this boot's plan came from.
  - This safe start is apart from the one after a run that did not end (000-KRN-0076). That one still reads the file, since it starts by itself and must not undo what the owner turned off.
- **Tests.** The host test `boot_abi` pins the range 16 to 20 and `boot_flags` zeroed before 20. The `modes` suite boots the file of 173-KRN-0035's test with S typed at the loader: the file is ignored, `windows` starts before `sysmon` again, `tts` and `video_gw` run, and `configuration` says `ignored`. Without the key the same file applies, as the first check shows.

## Related

[173](173-boot-services-configuration.md), [173-KRN-0035](../issues-done/173-KRN-0035-init-reads-the-service-configuration.done).
