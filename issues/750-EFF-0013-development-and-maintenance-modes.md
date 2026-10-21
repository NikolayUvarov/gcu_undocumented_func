# 750-EFF-0013 — Effector runs only in the development and maintenance modes: on by a word and by a command, kept across reboots

**Type:** design, `init`, shell, gateway · **Owner:** `EFF` · **Priority:** P1 · **Status:** open · **Blocked by:** `KRN`: `init` follows the mode (the request of 2026-10-21 on `internal-dev-messages`); `APP`: the `mode` command (the same request) · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-1.6, MC-3.8, MC-8.1, MC-10.2, MC-12.3, MC-12.6

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

1. **One file holds the mode.**
   - `development` and `maintenance` are words in one file on the boot volume. The proposed name is `data/mode.conf`, read with the service configuration of 173-KRN-0035.
   - Without either word, the machine is in its normal mode.
   - The word is written either when the disk is written (the maintainer's script, as `--link-dev` writes `link.conf`'s `dev`) or by the command (step 3).
   - The file is the state. It survives every reboot until the command, or the owner editing the volume, removes the word.
   - The pattern is the link's `dev` word ([800-KRN-0139](800-KRN-0139-the-links-developer-mode.md)).
2. **`init` follows the mode** (`KRN`).
   - **With either word,** `init` starts `effector` and `effector_gw` at boot and restarts them as it restarts other boot services.
   - **Without one,** it starts neither and makes no Effector grants at all.
   - **When the command changes the mode at run time,** both services start at once or stop at once, with no reboot. Stopping ends the session.
3. **The command** (`APP`, in the shell and in `msh`).
   - `mode` shows the mode and who set it.
   - `mode development on|off` and `mode maintenance on|off` write the file and tell `init`.
   - Turning a mode on asks the operator at the local console for confirmation. The question shows what Effector may then do. Turning a mode off needs no confirmation.
   - This replaces 750-APP-0093's `effector enable|disable`.
4. **The Mind's request** (MC-8.1, MC-12.6).
   - The Mind may turn the maintenance mode on when it needs help from outside. It goes through the same switch, with its own badge and a reason, and has no other way in.
   - The change is shown and audited like the operator's, and the operator may turn the mode off.
   - The Mind's component does not exist yet. The badge and the request are defined now, and the Mind holds the badge once it exists.
5. **Visible.**
   - While a mode is on, the screen says so.
   - Every change is one audit record: who, which mode, on or off, and the reason (MC-10.2).
6. **What Effector may do in a mode** (`EFF`).
   - The aim is to manage every aspect of the OS during development. The gateway's actions grow toward that: services, files, packages, updates, reboot, diagnostics, test-account scripts and debugging. Each action goes through the gateway and its audit.
   - The owner's policy still bounds what Effector does inside a mode.
   - Before the gateway widens anything, this task settles with the maintainer which of the policy's protected services, trees and forbidden grants stay protected in the development mode.
   - `agent on|off` repeats what the mode now says. It is dropped in the policy's next grammar version.
7. **The records.**
   - This design goes into 750, the threat analysis ([docs/effector/README.md](../docs/effector/README.md)) and [docs/effector/policy.md](../docs/effector/policy.md) now.
   - `KRN` and `APP` get the requests.
   - 0006's boot service and 0007's gateway follow it.

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
