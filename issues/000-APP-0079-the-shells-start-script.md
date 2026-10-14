# 000-APP-0079 — The shell's start script, from the boot volume's root

**Type:** tools (`shell`, `msh`) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open (planned) · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-10.2, MC-12.1

**Recorded by:** the tools track (APP), 2026-10-14, from the assurance track's request on `internal-dev-messages` (`APP/2026-10-14T1621Z-ASR-wifi-join-without-a-screen-and-a-start-script.md`), for [550-ASR-0007](550-ASR-0007-wifi-join-tests.md): a test on the MacBook Pro that runs at boot, without typing.

## Problem

Nothing runs at boot on the maintainer's request: the `diag/` scripts run only when typed in the shell. A hardware test that needs no keyboard has no way to start.

## Plan

- **Where the script lives.** The shell runs `autorun.msh` from the boot volume's root once at its start, when the file exists.
  - The request asked for `data/autorun.msh`. The tools track proposes the root instead: no program in the system can write there, only the shell's commands on `ram:`, `log:` and `data/`.
  - Under `data/`, any program the shell lends its file client (`fm`, a program that asks for `files`) could leave a script that runs at every boot with the shell's authority. Its `requires:` line is its own declaration, not an authorization (MC-3.11).
  - At the root, only whoever writes the disk can put the script there: the maintainer's `build-and-write.sh` on their own machine.
- **How it runs:**
  - in console 1, as `msh autorun.msh` would, with the script's `requires:` line limiting what it may do, as for any script;
  - after the shell's first frame, so the screen shows it;
  - the shell logs `[SHELL] AUTORUN autorun.msh SHA256 <hex>` before it, and `[SHELL] AUTORUN ENDED <status>` after it;
  - a key the user presses while it runs stops it, as Esc stops a script.
- **Without the file**, nothing changes and nothing is logged.
- **Not on every machine.** Images made by the build carry no `autorun.msh`, and the shell never writes one.
- **The help text,** `docs/tools` and `docs/msh.md` (EN, RU).

## Acceptance criteria

- **The `shell` suite:**
  - with `autorun.msh` at the disk's root, it runs once at start, and its line and the hash are in the log;
  - without it, the log has no `AUTORUN` line;
  - a script that needs a word its `requires:` lacks fails as it would when typed;
  - the shell's file client cannot write the root (`write /autorun.msh x` is refused).
- **The MacBook Pro** (550-ASR-0007): `build-and-write.sh --wifijoin` puts the script at the root, and the boot log shows it ran.

## Related

[550-ASR-0007](550-ASR-0007-wifi-join-tests.md) (the tests), [550-APP-0078](550-APP-0078-wifi-join-without-a-screen.md) (`wifi join`), [094](../issues-done/094-shell-script-language.done) (`msh`), [000-APP-0066](../issues-done/000-APP-0066-cd-pwd-and-tab-completing-paths.done) (scripts by name).
