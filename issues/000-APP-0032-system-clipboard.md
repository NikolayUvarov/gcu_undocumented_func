# 000-APP-0032 — The system clipboard: text copied in one program, pasted in another

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** in progress (the service and its clients written and host-tested, in `main` since round 124, 267ad266, 2026-11-12; `init` starting it and the QEMU check left) · **Blocked by:** `init` starting the service and giving the shell its client with `CAP_BADGE` ([requests-KRN.md](requests-KRN.md), APP's request of 2026-11-12); `SLOT_CLIPBOARD` and `REQUEST_CLIPBOARD` are in the ABI since [211-KRN-0058](../issues-done/211-KRN-0058-slots-for-the-shell-and-the-clipboard.done) · **Main task:** — (the tools plan's phase T4, [docs/tools §5](../docs/tools/README.md)) · **Roadmap:** track G · **Constitution:** MC-3.3, MC-11.5, MC-12.4

## Problem

Each program keeps its copied text to itself: `edit`'s Ctrl+C/X/V, `fm`'s command line, the shell's line. Nothing can be copied from `view` into `edit`, or from one file's editor into another's.

## Plan

- **`idl/clipboard.wit`** 1.0:
  - `set(text)` replaces the text;
  - `get()` returns it, with its generation;
  - text up to 64 KiB, in chunks of a message each.
- **`clipboard`**, a service of the tools track: it holds the text in memory only, never on a disk.
- **Who may read.** A program reads the clipboard only when lent a client, and only as the user's paste.
  - The shell lends a client only to a program that asks (`REQUEST_CLIPBOARD`), and in a script only under `requires: clipboard`.
  - Which program may `get` at a given moment is decided here, with the window broker and the shell, before the service is written. Two candidates: the program with the keyboard focus, or a badge per client that the shell enables while that program is in front.
- **Programs:**
  - `edit`, `view` (copy), `fm`'s command line and the shell's line;
  - `console`;
  - `wm` pastes into the window in front.
  - Without a client, each keeps its own buffer, as now.

## Acceptance criteria

- **Host tests:** the service's chunks, limits and generations.
- **QEMU (tools suite):**
  - text copied in `edit` is pasted into the shell's line and into a second `edit`;
  - a program that did not ask for the clipboard holds nothing in the slot;
  - a program in the background cannot read it (whichever rule is chosen above).

## Progress (2026-11-12)

- **Who may read, decided:** the program in front, as the managers name it. The shell (for the console shown) and the window manager it started (for its window in front; none while its menus or Settings have the keys) hold copies badged `BADGE_MANAGER` and call `front`; a program's copy, badged `BADGE_PROGRAM`, is answered only while its PID is the one named. A badge per client, enabled by the shell, would need the shell to know each window's program in front, which only the window manager knows.
- **`idl/clipboard.wit` 1.0:** `set-part`, `get-part`, `generation`, `front`. The text is up to 65536 bytes, carried in parts of 4000 bytes split where a character ends: the generated record holds its text inline, so a 64 KiB one would not fit a program's 64 KiB stack. `mind::clipboard` joins them (`copy`, `paste`, read again from the start when the text changes meanwhile).
- **`clipboard/`** (`clipboard.elf`): the service, its rules in `clipboard/src/state.rs`. The text in memory only, zeroed when replaced; each text set and read logged with its length and generation, never the text.
- **The clients:**
  - **the shell:** mints its manager copy from `init`'s client in `SLOT_CLIPBOARD`, names the program in front of the console shown as it changes, lends a program's copy for `REQUEST_CLIPBOARD` (a script's under `requires: clipboard`) and a window manager one that may badge; its line pastes its first line with Ctrl+V or Shift+Insert and copies itself with Ctrl+Insert;
  - **`wm`:** asks for it, mints its manager copy, names the program of the window in front, and lends its windows' programs their copies;
  - **`edit` and `fm`'s editor:** Ctrl+C/X/V through it (`Outcome::Copy`, `Outcome::Paste`), their own copy where it does not answer; `fm`'s store wrapper (`WithStore`) passes the clipboard on to its disk;
  - **`view`:** Ctrl+C copies the line found, else the top one; **`console`** and **`fm`'s command line:** Ctrl+V or Shift+Insert paste the first line.
- **Tests:** host `tests/clipboard_host.rs` (who may, the parts in order and within 64 KiB, generations, characters' ends; 3 tests), `tests/edit_host.rs` (the editor's outcomes), `tests/viewer_host.rs`, `tests/console_host.rs`, `tests/fm_host.rs` (through the store wrapper too), the IDL fuzz test with the new receiver.
- **Left:**
  - `init` starting the service (KRN's 000-KRN-0235), with `clipboard` in `BOOT_SERVICES`; then `clipboard.elf` back in `02_build.sh`'s list, left out until then since `wm`'s menu lists a program on the disk that opens no window;
  - the QEMU `tools` suite's check once it runs: `edit`'s text pasted into the shell's line and a second `edit`, a program without the request holding nothing in the slot, a program behind refused;

## Related

[requests-KRN.md](requests-KRN.md), [docs/tools §4.2, §5](../docs/tools/README.md), [000-APP-0030](../issues-done/000-APP-0030-syntax-colours-in-edit.done).
