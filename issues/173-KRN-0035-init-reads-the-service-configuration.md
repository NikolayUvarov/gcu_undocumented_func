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

## Progress

2026-10-24, the kernel track:

- **The file.** `init/src/services.rs` parses `data/services.txt` without a heap: `disable`, `enable` and `after`, `#` comments, at most 4 KiB of ASCII. A line is refused and logged with its number and why when it names an essential service, an unknown one, is malformed, or orders a service against init's own rules or in a cycle. An image's name covers its instances (`virtio_net` and `virtio_net#1`).
  - What the file may change: the services after `vfs_server` but `loader`, `link`, the Effector services (data/mode.conf decides them), `shell` and `tpm` (it seals the device key).
  - `NEEDS`: `netpolicy`, `tls` and `ssh` need `netstack`, `tls` and `ssh` `keystore`, `tts` `audio_gw`. Turning one off turns off what needs it.
  - `BEFORE`: a service whose grants init gives only when another runs starts after it (`wlan` after its radio and `keystore`, `netstack` after its cards and `wlan`, `updater` after `tls`, `parse` and `netpolicy`, the shell after the services it gets clients of only when they run). A service that waits holds back what starts after it by these rules.
- **The boot.** init reads the file at the first service it may change, once the boot volume is mounted, through a reader's client of its own.
  - A service the file turns off, or that needs one that is, is prepared as a safe start's are, and `svc start` starts it. Its clients answer `ERR_PEER` until then: the shell, the updater, the station and the Effector session now get clients of prepared services as of running ones.
  - One that waits starts as soon as what it waits for was tried. The log names each line refused, the counts, what waits for what and what is off and why. A disabled service does not count against the boot's health.
  - A safe start still reads the file: it leaves off what the file does as well as its own; ignoring the file is [173-KRN-0036](173-KRN-0036-safe-start.md)'s key.
- **`boot-plan`** in `init.wit` 1.9 (1.4 was taken by `boot`): each service's place, enabled, essential, what it waited for, and why it did not start (`disabled`, `needs <service>`, `no device`, `no image`, `safe start`, `mode`, `no link.conf`, `failed`).
- **init's stack.** init now asks for 256 KiB in `PT_GNU_STACK` (`init/linker.ld`), and the boot loop is a function of its own.
  - Its boot frame holds every service's plan. It was 43 KiB before this change, and a mode's start of `effector_gw` reached 60 of its 64 KiB.
  - The first build of this change overflowed it: a page fault in init, in the `modes` suite.
- **Tests.**
  - `tests/services_host.rs` runs the parser over the real boot order: the rules name real services in their order, the maintainer's example, what needs what, instances, refusals, contradictions and cycles, a long and a foreign file.
  - The `modes` suite (`services_config_check`) boots a file that turns off `tts` and `video_gw` and starts `windows` after `sysmon`: the log, the order of the STARTED lines, `boot-plan` as the gateway's stand-in asks it, and `svc start tts`. It also boots a file with `disable logd`, an unknown name and a malformed line: the three lines named, the boot as without the file.
  - Passed in this session's container (QEMU TCG): the x86 `modes`, `services`, `tts` and `watchdog` suites, and the aarch64 `modes` suite.

## Related

[173](173-boot-services-configuration.md), [173-KRN-0036](173-KRN-0036-safe-start.md).
