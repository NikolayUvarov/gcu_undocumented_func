# 750-EFF-0013 — Effector runs only in the development and maintenance modes: on by a word and by a command, kept across reboots

**Type:** design, `init`, shell, gateway · **Owner:** `EFF` · **Priority:** P1 · **Status:** open · **Blocked by:** `KRN`: [750-KRN-0143](750-KRN-0143-init-follows-the-development-and-maintenance-modes.md) (`init` follows the mode; on `main` d3caa1f as a task); `APP`: the `mode` command (the request of 2026-10-21 on `internal-dev-messages`) · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-1.6, MC-3.8, MC-8.1, MC-10.2, MC-12.3, MC-12.6

## Problem

The maintainer's decision (2026-10-21):
- The Effector module for MIND Core is how Effector manages **every aspect of the OS during development**.
- That needs high privileges and is potentially risky. So Effector is on **only in the development mode and in the maintenance mode**. A mode is turned on explicitly, by the operator, or by the Mind when it needs help from outside.
- In these modes **Effector is a service that runs all the time.**
- **Both modes are turned on by a word and by a command**, so that the mode survives any number of reboots.

The plan until now is different:
- [750](750-effector-agent.md) and [750-KRN-0119](750-KRN-0119-boot-images-and-grants-for-effector.md) start both services when the owner enables them in `data/services.txt` (173-KRN-0035).
- The policy's `agent on|off` ([docs/effector/policy.md](../docs/effector/policy.md)) decides what they may do.
- Neither depends on a mode.

## Plan

1. **One file holds the mode** (`KRN`'s choices in 750-KRN-0143, 2026-10-21).
   - `development` and `maintenance` are words in `data/mode.conf` on the boot volume. `init` reads it after `vfs_server` starts, through a client confined to that one file. The bootloader does not read it, and no boot information changes.
   - Without either word, the machine is in its normal mode.
   - The word is written either when the disk is written (the maintainer's script, as `--link-dev` writes `link.conf`'s `dev`), or by `init` itself on a run-time switch (step 2).
   - The file is the state. It survives every reboot until the switch, or the owner editing the volume, removes the word.
   - The pattern is the link's `dev` word ([800-KRN-0139](800-KRN-0139-the-links-developer-mode.md)).
2. **`init` follows the mode** (`KRN`).
   - **With either word,** `init` starts `effector` and `effector_gw` at boot and restarts them as it restarts other boot services.
   - **Without one,** it starts neither and makes no Effector grants at all.
   - **At run time** the switch is `init.wit` 1.5 `set-mode(mode, on, reason)`: `init` writes the file and starts or stops both services at once, with no reboot, so the file and the services never disagree. Stopping ends the session. `mode()` reads the mode, who set it and when.
3. **The command** (`APP`, in the shell and in `msh`).
   - `mode` shows the mode and who set it.
   - `mode development on|off` and `mode maintenance on|off` call `set-mode` with the shell's `BADGE_MODE`.
   - Turning a mode on asks the operator at the local console for confirmation. The question shows what Effector may then do. Turning a mode off needs no confirmation.
   - This replaces 750-APP-0093's `effector enable|disable`.
4. **The Mind's request** (MC-8.1, MC-12.6).
   - The Mind may turn the maintenance mode on when it needs help from outside. It goes through the same switch, with `BADGE_MIND` and a reason, and has no other way in. That badge may only turn the maintenance mode on; any caller without one of the two badges is refused.
   - The change is shown and audited like the operator's, and the operator may turn the mode off.
   - The Mind's component does not exist yet. The badge and the request are defined now, and the Mind holds the badge once it exists.
5. **Visible.**
   - While a mode is on, the screen says so.
   - Every change is one line in the system log from `init`: the badge, the caller's PID, the mode, on or off, and the reason (MC-10.2). The gateway's audit takes it from there or from `mode()`.
6. **What Effector may do in a mode** (`EFF`).
   - The aim is to manage every aspect of the OS during development. The gateway's actions grow toward that: services, files, packages, updates, reboot, diagnostics, test-account scripts and debugging. Each action goes through the gateway and its audit.
   - The owner's policy still bounds what Effector does inside a mode.
   - Before the gateway widens anything, this task settles with the maintainer which of the policy's protected services, trees and forbidden grants stay protected in the development mode.
   - `agent on|off` repeats what the mode now says. It is dropped in the policy's next grammar version.
7. **The records.**
   - This design goes into 750, the threat analysis ([docs/effector/README.md](../docs/effector/README.md)) and [docs/effector/policy.md](../docs/effector/policy.md) now.
   - `KRN` and `APP` get the requests.
   - 0006's boot service and 0007's gateway follow it.

## Open question for the maintainer (from `KRN`)

Does a safe start (000-KRN-0076) still start the services in the maintenance mode? For: help from outside is what a machine that failed to boot needs. Against: a safe start trusts as little as it can.

## Acceptance criteria

- **In QEMU (x86_64 and aarch64):**
  - Without a mode word, neither service runs, and no process holds an Effector grant (`stat caps`).
  - With `development` written when the disk was written, both run from boot. Each is started again after it is killed.
  - `mode maintenance on`, confirmed at the console, starts both without a reboot.
  - After three reboots, the mode still holds and both services run.
  - `mode … off` stops both, and the session ends.
  - A change made through the Mind's badge is shown and audited. A request from a holder without the badge is refused.
- **The threat analysis and the profile** say that the services run only in these modes, and on which configurations that was tested (MC-12.3).

## Related

[750](750-effector-agent.md), [750-KRN-0119](750-KRN-0119-boot-images-and-grants-for-effector.md) (to follow the mode), [750-APP-0093](750-APP-0093-the-shells-effector-command.md) (`mode` instead of `effector enable|disable`), [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md) (the same pattern for the link), [750-EFF-0006](750-EFF-0006-session-service.md), [750-EFF-0007](750-EFF-0007-gateway-service.md).
