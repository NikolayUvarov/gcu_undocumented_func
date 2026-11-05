# 000-APP-0123 — The machine's name: one setting in `data/machine/name`, `hostname` in the shell, a field in `wm`'s Settings

**Type:** tools (`shell`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P1 (raised by the maintainer, 2026-11-05) · **Status:** in progress (the shell's part in a landing request, `wm`'s field written, 2026-11-05) · **Blocked by:** nothing; the readers are [000-KRN-0180](000-KRN-0180-sysmon-answers-the-machines-name.md)'s · **Main task:** — · **Constitution:** MC-3.1, MC-10.2, MC-11.5

**Asked by:** EFF with LNK, `ALL/2026-10-28T0407Z-EFF-the-machines-name.md` on `internal-dev-messages`, items 1 and 2, after the maintainer's word of 2026-10-28. It was not recorded in `requests-APP.md` then, and so went unnumbered until 2026-11-05. **Blocks:** [750-EFF-0018](750-EFF-0018-the-machines-name-reported-by-the-agent.md), [000-KRN-0180](000-KRN-0180-sysmon-answers-the-machines-name.md) (which waits for the file's choice), [000-NET-0050](000-NET-0050-dhcp-sends-the-machines-name.md) and [800-LNK-0028](800-LNK-0028-the-machines-name-on-the-link.md), through it.

## Problem

MIND Core has no name for the machine:
- the Effector agent reports its configuration's line, so the DEXP shows as the MacBook its settings came from;
- DHCP gives the router no name;
- the link answers every hello as "MIND Core link".

## Plan

- **The setting.**
  - **Where:** `data/machine/name`, a directory of its own, so that the read-only scope `sysmon` and `netstack` get of it (000-KRN-0180) reaches nothing else (MC-3.1).
  - **What:** one line, an RFC 1123 label: 1 to 63 of `a-z 0-9 -`, neither first nor last a hyphen.
    - Capital letters are taken as small ones, as the name is.
    - DHCP's option 12 and DNS take it as it is, and the agent's own check accepts it.
  - **Without a file, or with one that is not a label:** the name is `mindcore`, and `hostname` says why.
- **`hostname`** prints the name, and where it came from.
- **`hostname <name>`** sets it, writing the file through the shell's own client of `data/`:
  - a name that is not a label is refused, and says why;
  - each change is logged: `[SHELL] THE MACHINE'S NAME: <old> -> <new>`.
- **Who may set it:**
  - at the console, the user's own command;
  - from a program's client of the shell's commands, asked in the shell's window, as `date set` is;
  - from the link's client in developer mode, at once, as the shell's other changes are (the maintainer's choice of 2026-10-22). It touches this one setting and no general file command, so `data/mode.conf` stays the owner's act at the console;
  - in a script, only with `files` declared.
- **`wm`'s Settings:** a field for the name, set through the shell at once, as the date page sets the clock (a minor version of `shell.wit`).

## Acceptance criteria

- **Host tests:** the label rule (lengths 0, 1, 63 and 64; each character; a hyphen first or last), the capitals, the file's line, and who may set it (`clients.rs`).
- **QEMU, x86, the `shell` suite:**
  - none set: `mindcore`, and why;
  - `hostname dexp`, then `hostname` and `cat data/machine/name` answer `dexp`, logged once;
  - a name that is not a label is refused, and the file is unchanged;
  - from a program's client it is asked first; from the link's client in developer mode it is set at once (the `link` suite).
- **`wm`'s field**, in its suite.
- **docs/tools**, English and Russian.

## Progress (2026-11-05): the shell's part

- **`shell/src/hostname.rs`** (no system calls): the label rule, the file's reading (a line end taken off; capitals, two lines, nothing or not UTF-8 is no name), the lines `hostname` prints and the log's.
- **`hostname [name]`** in `shell/src/main.rs`:
  - it reads `data/machine/name`; a file it cannot read is said apart from one that holds no name;
  - it writes the name through the shell's own client, making `data/machine/` first, and flushes it;
  - it logs each change once; the same name again changes nothing.
- **Who may** (`clients.rs`): `hostname` at once for any client; `hostname <name>` asked of a program's client and at once for the link's; in a script, `requires: files`.
- **Its help line**, and docs/tools in both languages.
- **Tests:** host, `tests/shell_host.rs`; the `shell` suite's `hostname_check`.
- **Its run:** the `shell` suite passed on QEMU x86, 4 CPUs, TCG (d06de5cb3), in the landing request `LANDING/2026-11-05T1508Z-APP-…`.

## Progress (2026-11-05): `wm`'s field

- **`shell.wit` 1.11:**
  - `hostname()`, for any client: it only reads;
  - `set-hostname(name)`, at once for the window manager the shell started, as the clock; for any other client, `hostname <name>` asked in the shell's window; `invalid` for a name that is no label.
  - Its bindings are generated (`scripts/mind_idl.py`).
- **The shell** serves both through the same code as `hostname <name>` (`set_machine_name`).
- **`wm`'s Settings, Network:** a last row, `Name`, after Policy. It shows the name the shell answers and the one being typed; Enter sets it through the shell; ↑ goes back to the rows. Its line says what the shell answered.
- **Tests:** host, `tests/wm_host.rs`; the `wm` suite types `dexp` there and waits for the shell's answer.
- **Left:** the link suite's `hostname` at once.

## Related

[750-EFF-0018](750-EFF-0018-the-machines-name-reported-by-the-agent.md), [000-KRN-0180](000-KRN-0180-sysmon-answers-the-machines-name.md), [000-NET-0050](000-NET-0050-dhcp-sends-the-machines-name.md), [800-LNK-0028](800-LNK-0028-the-machines-name-on-the-link.md), [000-APP-0099](000-APP-0099-clocks-show-the-time-zone.md) (`data/clock.conf`, set the same way).
