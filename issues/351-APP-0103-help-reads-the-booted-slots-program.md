# 351-APP-0103 — `help <name>` reads the program from the booted slot

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Roadmap:** track C · **Constitution:** MC-9.3, MC-12.1

**Recorded by:** the tools track, 2026-10-22, from KRN's request in [requests-APP.md](requests-APP.md) (for [351-KRN-0151](351-KRN-0151-applications-from-the-booted-slot.md) and 351-UPD-0023).

## Problem

- **Where `loader` takes programs now.** On a disk booted from a slot (`MIND/A/` or `MIND/B/`) that holds applications, `loader` opens a program by its bare name there, and lists the slot's in `LIST` (351-KRN-0151).
- **Where the shell reads them.** `programs::about` (`help <name>`, `<name> --help` for a program with a screen) opened `<name>.elf` at the root.
- **So `help` read another build's text,** or none once a release dropped a program.

## Plan

- **The same test as `loader`'s.** The booted slot holds applications when it holds any `*.elf` that is neither `kernel.elf` nor a boot file.
- **Looked for once,** at the first `help <name>`, so the shell's start does not wait for it. The slot comes from `BootInfo.boot_slot`.
- **The file read** is `<slot>/<name>.elf` when the slot holds applications, else `<name>.elf` at the root, as before.
- **`list -l`** takes its names and sizes from `loader`'s `LIST`, which already follows the slot. Its lines come from `about`.

## Acceptance criteria

- **The `linkupdate` suite's `slot_applications_check`:**
  - a slot without applications: `help uptime` shows the root's text;
  - a slot that holds `probe.elf` (uptime's file): `help probe` shows its text, and `help uptime` names no such program.
- **Docs:** `docs/tools/README.md` and `README_RU.md`.

What `help` shows is evidence for the images tested only (MC-12.1).

## Related

- [351-KRN-0151](351-KRN-0151-applications-from-the-booted-slot.md): the loader's side.
- [351-APP-0088](../issues-done/351-APP-0088-files-reads-the-booted-slots-kernel.done): `files` reads the booted slot's kernel.
