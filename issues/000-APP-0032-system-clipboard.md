# 000-APP-0032 — The system clipboard: text copied in one program, pasted in another

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open · **Blocked by:** `SLOT_CLIPBOARD`, `REQUEST_CLIPBOARD` and `init` starting the service ([requests-KRN.md](requests-KRN.md)) · **Main task:** — (the tools plan's phase T4, [docs/tools §5](../docs/tools/README.md)) · **Roadmap:** track G · **Constitution:** MC-3.3, MC-11.5, MC-12.4

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

## Related

[requests-KRN.md](requests-KRN.md), [docs/tools §4.2, §5](../docs/tools/README.md), [000-APP-0030](../issues-done/000-APP-0030-syntax-colours-in-edit.done).
