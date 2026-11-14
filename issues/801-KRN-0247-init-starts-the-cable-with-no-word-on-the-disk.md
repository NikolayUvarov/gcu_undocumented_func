# 801-KRN-0247 — `init` starts the cable with no word on the disk: `link` without `link.conf`, its clients at every start

**Type:** kernel (`init`) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S6) · **Roadmap:** stage V · **Constitution:** MC-2.3, MC-9.2, MC-12.3

## Problem

LNK's request (`KRN/2026-11-14T0944Z-LNK-801-init-starts-link-with-no-link-conf-and-grants-its-clients.md`), for [801-LNK-0044](801-LNK-0044-the-cable-updates-everything-with-no-permission-words.md).

The maintainer's decision of 2026-11-14 (801, S6): «effector485 должен уметь обновлять весь код, любой абсолютно без всяких разрешений». The cable updates all code with no permissions. It is for critical cases and may be absent; releases stay signed.

`init` still asked two words of the disk:
- **No `link.conf`, no cable.** `usb_serial`, `uart_serial` and `link` were not started where the bootloader found no `link.conf` at the boot volume's root (`[INIT] link NOT STARTED: NO LINK.CONF`). A `link.conf` of 512 bytes or more counted as none too.
- **No `dev`, no clients.** Without the word `dev` in `link.conf`, `link` got none of these:
  - the system monitor;
  - `init`'s lifecycle client (`reboot`, `replace`);
  - the launch-only policy client;
  - the Wi-Fi station's client;
  - the shell's commands;
  - `data/firmware/`;
  - the input privilege.

  So a machine whose disk lacked the word could not be restarted or repaired over the cable. `link-console` in `data/mode.conf` was also left out without `dev`.

## Plan

- **`link` starts without `link.conf`.**
  - `usb_serial` and then `link` start whenever `usb_host` runs, with or without the file.
  - The line's choice stays: `uart_serial` only with `uart` in `link.conf`, `usb_serial` otherwise.
  - With no converter both wait idle: `usb_serial` looks once a second, and `link` logs that it waits.
- **The cable's clients at every start of `link`, with or without `dev`.** These are the clients that `dev` gave before.
  - The input privilege is still withheld in a safe start (800-KRN-0191): whoever asked for the safe start is at the machine.
  - `init` logs `[INIT] link: THE CABLE'S CLIENTS (801-LNK-0044): …`. The word `dev` then means nothing to `init`, as it already means nothing to `link`.
- **`link-console` counts from `data/mode.conf` without `dev`.** It stays the owner's word there.
- **What stays bounded** (MC-2.3) is the badge `init` gives each client:
  - `netpolicy` refuses the launch-only client any change (800-KRN-0142);
  - the shell applies the link's rule to `BADGE_SHELL_LINK` (800-APP-0102): never `mode`, never a policy change;
  - an update through the cable is checked by the updater as any source's (MC-9.2–9.4).
- **The threat model already says it** (MC-12.3): LNK's entry, which this task updates, treats the cable as physical access.
- **Not changed here:**
  - The boot plan's `no link.conf` stays the text for the line `link.conf` did not choose. It is one of `init.wit`'s documented values, so renaming it waits for the next `init.wit` version.
  - The comments in `idl/init.wit` (lines 47, 64, 69) that say `dev` wait for the same version.
  - The shell's `LINK CONSOLE: ON (… DEV IN LINK.CONF)` line is APP's.
  - `docs/tools`, `docs/write-disk` and `scripts/build_and_write.sh`'s `--link-dev` are APP's and LNK's.

## Acceptance criteria

1. **The `linkupdate` suite, x86 and aarch64:**
   - `cable_without_words_check` (LNK's) no longer prints `SKIP: cable without words: init starts link only when the boot volume holds link.conf` or `SKIP: cable without words: reboot: …`;
   - on a disk with no `link.conf`, `link` serves at 460800 bit/s, and the machine restarts on `reboot` over the link and serves again;
   - the suite's own image with no `dev` holds the cable's slots 7, 10, 11, 19 and 30.
2. **The `link` suite, on a disk whose `link.conf` has no `dev` (`address=5`):**
   - `link` holds the clients above;
   - its `FROM INIT:` line names `REBOOT SHELL INPUT FIRMWARE`;
   - `sh ps`, keys, `replace` and `reboot` work.
3. **The safe start still gives no input privilege.**
4. **The `modes` suite:** `link-console` counts with no `link.conf`.
5. **The documents:** `docs/profile` (`bootstrap.md` and `threat-model.md`) and the README say what `init` does now.

## Related

[801](801-the-bench-stays-manageable.md), [801-LNK-0044](801-LNK-0044-the-cable-updates-everything-with-no-permission-words.md), [801-UPD-0039](801-UPD-0039-the-updater-always-reads-the-cables-directory.md) (the updater's half), 800-KRN-0139, 800-KRN-0191.
