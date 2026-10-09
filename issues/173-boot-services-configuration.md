# 173 — Which boot services start, and in what order: a configuration and a tool for it

**Type:** main task (plan) · **Owner:** kernel track (`init`, the bootloader's flag), with the tools track (`svc`) · **Priority:** P2 · **Status:** open (a plan) · **Blocked by:** — · **Roadmap:** C6 (supervision), K1 (policy out of the kernel) · **Constitution:** MC-3.12, MC-6.1, MC-6.2, MC-10.2

Asked by the maintainer (2026-10-09): a tool to manage the start of services: turn them on and off, and set their order when needed.

## Problem

`init` starts every boot service whose image and device are present, always in the order of `BOOT_SERVICES` (`common/abi.rs`). The user cannot keep a service from starting: on a machine without speakers or a camera, `audio_gw`, `tts` and `video_gw` still start, and so does `netstack` with no network. Nor can the user change the order.

At run time `svc` can stop and restart a service (`idl/init.wit` 1.1), but nothing of that outlives a reboot.

## Plan

### 1. The configuration (`init`, `KRN`)

- **A file the user may write:** `data/services.txt` on the boot disk, one directive a line, `#` for comments:
  - `disable <service>`: init does not start it at boot; `svc start` still can.
  - `enable <service>`: the default, written to undo a `disable`.
  - `after <service> <other>`: start it only once `<other>` started. The only reordering allowed: init keeps its own dependencies, and a directive that contradicts one is refused.
- **What it may touch.** In the first step only the services init starts after `vfs_server`, because init cannot read the file before then:
  - `gpio`, `audio_gw`, `tts`, `video_gw`, `virtio_net` and its instances, `netstack`, `netpolicy`, `keystore`, `tls`, `windows`, `sysmon`, `blockstore`, `updater`.
  - The drivers, `vfs_server` and the core (`init`, `logd`, `rtc`, `compositor`, `loader`, `shell`) are essential.
  - A directive naming an essential service is refused and logged; so are unknown names and malformed lines. A bad file never stops the boot.
- **Dependencies are init's.** For example `netpolicy` and `tls` need `netstack`, `tls` needs `keystore`, and `windows` needs the compositor. Disabling a service disables those that need it, and init logs which.
- **The plan is visible.**
  - init logs the boot plan: the order, what was left off and why.
  - `idl/init.wit` gains a minor version with `boot-plan`: each service, whether it is enabled, essential or left off by a dependency, and its place. `list` stays as it is.
- **Health and trial boots.** A disabled service does not count against the boot's health ([351-KRN-0014](../issues-done/351-KRN-0014-trial-boot-and-confirmation.done)): init confirms a trial when every *enabled* service started.

### 2. A safe start (bootloader and `init`, `KRN`/`PRT`)

- A key held at the bootloader's start (for example `Esc` during its 5-second pause, which shows a line saying so) sets a flag in `BootInfo`.
- With the flag set, init ignores `data/services.txt`, starts every service, and logs why. The `BootInfo` field is an ABI change.
- This is the way back from a configuration that leaves a machine without its keyboard or network.

### 3. The tool (`svc`, `APP`; a request in `requests-APP.md`)

- `svc boot` shows the boot plan from init: order, enabled, essential, left off and why, and how the last boot went.
- `svc enable <service>`, `svc disable <service>` and `svc after <service> <other>` edit `data/services.txt` through the shell's file client, check the result against `boot-plan`, and say that it applies at the next boot. `svc reset` removes the file.
- It refuses what init would refuse: essential services, unknown names, contradicted dependencies.
- The help screen and `docs/tools` describe it.

### Later

- Configuring the drivers and the core needs the configuration before `vfs_server` mounts. The bootloader could read it from the boot volume and pass it in `BootInfo`, the way it passes the manifest's digest.
- A service started on demand, at its first request, instead of at boot.

## Tasks

| Task | Owner | What |
|---|---|---|
| [173-KRN-0035](173-KRN-0035-init-reads-the-service-configuration.md) | `KRN` | init reads and applies `data/services.txt`; `boot-plan` in `init.wit` |
| [173-KRN-0036](173-KRN-0036-safe-start.md) | `KRN`, with `PRT` for the bootloader | A safe start: a key at the bootloader, a flag in `BootInfo`, the configuration ignored |
| request in [requests-APP.md](requests-APP.md) | `APP` | `svc boot`, `enable`, `disable`, `after`, `reset` |

## Acceptance criteria

In QEMU (x86 and aarch64):

- `svc disable tts` and a reboot: `tts` does not start, `svc boot` says so, and `svc start tts` still works.
- `svc after windows sysmon`: the next boot starts `sysmon` before `windows`.
- Disabling `logd` is refused by `svc` and, written into the file by hand, by init.
- A malformed file boots like no file, with the lines named in the log.
- The safe start ignores the file.

## Related

`idl/init.wit`, [170](../issues-done/170-supervisor-without-usable-privileges.done), [351-KRN-0014](../issues-done/351-KRN-0014-trial-boot-and-confirmation.done), the `svc` tool ([070](../issues-done/070-svc-lifecycle.done), `docs/tools` §4).
