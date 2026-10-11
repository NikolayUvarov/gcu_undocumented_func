# 173-KRN-0035 — init reads and applies the service configuration; the boot plan in `init.wit`

**Type:** kernel (`init`) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [173](173-boot-services-configuration.md) · **Constitution:** MC-3.12, MC-6.1, MC-10.2

## Problem

See [173](173-boot-services-configuration.md): init starts every boot service in a fixed order, and nothing the user sets outlives a reboot.

## Plan

- **Reading the file.** After `vfs_server` starts, init reads `data/services.txt` through a read-only client of its own:
  - at most 4 KiB, ASCII, one directive a line (`disable`, `enable`, `after`);
  - every refused line is logged with its number and why.
- **Applying it.** The services after `vfs_server` start in `BOOT_SERVICES` order, adjusted by `after`.
  - The essential services and init's own dependencies always win.
  - A disabled service is recorded as such: `list` shows it stopped, not failed, and `svc start` still starts it.
- **`init.wit` 1.4: `boot-plan`.** Each service with its place, `enabled`, `essential`, and why it is off: `disabled`, `needs <service>`, or `no device`. The bound is 64 services, as `list` has.
- **Health.** The trial boot is confirmed when every enabled service started.
- **Tests (QEMU, x86 and aarch64):**
  - a file written on the host into `data/` disables `tts` and `video_gw` and orders `sysmon` before `windows`; the boot starts what it says, and `boot-plan` reports it;
  - a file with `disable logd`, an unknown name and a malformed line boots as without it, and the log names the three lines.

## Acceptance criteria

The tests above pass, and a disabled service still starts with `svc start`.

## Related

[173](173-boot-services-configuration.md), [173-KRN-0036](173-KRN-0036-safe-start.md).
