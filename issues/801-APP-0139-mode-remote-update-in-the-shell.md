# 801-APP-0139 — `mode remote-update` and `mode link-remote-update` in the shell, and the link's console taking the first only with the second

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** — (`init.wit` 1.11's words are in `main`, 751-KRN-0185) · **Main task:** [801](801-the-bench-stays-manageable.md) (S7), for [751](751-system-updates-at-a-developers-request.md) · **Roadmap:** the development of MIND Core on real hardware; stage V · **Constitution:** MC-3.4, MC-9.3, MC-12.6

**Asked by:** EFF on 2026-10-29 (`APP/2026-10-29T0959Z-EFF-a-request-mode-remote-update-in-the-shell.md` on `internal-dev-messages`), and LNK for 801's S7 on 2026-11-14 (`ALL/2026-11-14T0847Z-LNK-801-the-bench-stays-manageable-your-parts.md`). EFF's request was not numbered when it came; it is numbered now, on 2026-11-14.

## Problem

- **`init` holds the owner's permission** for a release from the Effector server, the word `remote-update` of `data/mode.conf` (751-KRN-0185, `init.wit` 1.11), and `link-remote-update`, the owner's leave for the link's console to switch it.
- **The shell cannot switch either.** `mode` knows only `development` and `maintenance`, so the word comes only from the disk as it was written. On the bench a disk rewrite was the only way to turn it on (801).
- **The link's console** (800-APP-0108) is refused every `mode`, so the owner's `link-remote-update` gives the cable nothing.

## Plan

- **At the console:** `mode remote-update on|off [reason]` and `mode link-remote-update on|off [reason]`, through `set-mode` with the shell's `BADGE_MODE`.
  - **On** asks at the local console first, naming what it allows: a release sent through the Effector server may be installed on this machine and the machine restarted into it.
  - **Off** does not ask.
  - `init` refuses `remote-update` on without a mode, and `link-remote-update` on without the link's console; the shell says why.
- **`mode` alone** shows both words beside the modes, and whether each counts now.
- **The link's console** (`Client::LinkConsole`):
  - `mode remote-update on|off` is taken while `init` reports `link-remote-update`; the shell asks `init` with its own badge and names the link as the source;
  - without that word it is refused, as every `mode` from the link is today;
  - `mode link-remote-update …` from the link is always refused, and so is every other `mode`.
- **The notice** above the first prompt and in the system log while `remote-update` counts: `REMOTE UPDATE ON: A RELEASE SENT THROUGH THE EFFECTOR SERVER MAY BE INSTALLED`, and whether the link may switch it.

## Acceptance criteria

- **Host tests (`tests/shell_host.rs`):** the command line and its limits, the state line, the question and the notice, and the link console's rule both ways.
- **QEMU (`modes` suite, x86):** `mode remote-update on` asked and answered N changes nothing, answered Y `init` counts it; `mode` shows it; the notice after a restart; `mode remote-update off`; refused without a mode.
- **Docs:** `docs/tools/README.md` (both languages) and the help line.

## Related

[801](801-the-bench-stays-manageable.md), [751](751-system-updates-at-a-developers-request.md), [751-KRN-0185](751-KRN-0185-the-remote-update-permission-in-init.md), [751-LNK-0030](751-LNK-0030-the-remote-update-words-on-the-disk-and-over-the-cable.md), [750-APP-0105](../issues-done/750-APP-0105-the-mode-command.done), [800-APP-0108](../issues-done/800-APP-0108-the-links-lines-as-the-consoles.done).
