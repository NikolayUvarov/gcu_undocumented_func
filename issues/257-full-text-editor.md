# 257 — The text editor, full-featured: `edit` grown into a programmer's and writer's editor

**Type:** main task, tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — (a system clipboard: a request to `KRN` or a clipboard service, stage 2) · **Roadmap:** track G (user services: the editor) · **Constitution:** MC-3.1, MC-5.1, MC-11.3, MC-12.1, MC-12.3

**Recorded by:** the tools track, 2026-10-23, at the maintainer's request: a text editor in two variants, a text one and a WYSIWYG one ([258](258-wysiwyg-document-editor.md)). This is the text one.

## Problem

`edit` (issue 067, `docs/tools/README.md` section 4.2) is a panel text editor in the mcedit and FAR style. It has:

- a piece table with a line index, files up to 8 MiB;
- UTF-8 with line endings kept;
- search, replace and go to line, undo and redo;
- selection with Shift and the editor's own clipboard;
- syntax colours for ten languages, the mouse, read-only mode.

What a full editor has and `edit` has not:

- **More than one file:** buffers, tabs, a split view.
- **Search:** regular expressions, across files, case and whole-word options, search history.
- **Editing:**
  - column (block) selection; multiple cursors;
  - indentation of a selection, comment toggling;
  - matching brackets, auto-indent;
  - sorting and joining lines, case conversion;
  - macros (record and replay).
- **Large files** past 8 MiB, read by window as `view` does.
- **Its hex mode,** a deviation of issue 067 (only `view` has one).
- **A system clipboard** shared with the shell, `fm` and other programs.
- **Files:** recent files, sessions (the open files and positions kept), backups and recovery after a crash, a file changed outside the editor noticed.
- **Line endings and encodings** converted on request; a BOM kept or dropped.
- **Settings** in a file (`data/apps/edit/edit.conf`): tab width, spaces or tabs, wrap, colours, key bindings.
- **For programmers:**
  - an outline of a file's functions or headings;
  - a jump to a line from `grep -n`'s or a compiler's output;
  - spell checking for Russian and English with dictionaries on the model disk.

## Plan

- **Keep `edit`'s keys and screen;** add, don't change. The F-key bar and the menu grow as the features do, each documented.
- **The core** stays a `no_std` module tested on the host (`tests/edit_host.rs`). Each feature's logic is host-tested before its screen.
- **In order:**
  1. buffers and a split view; regular-expression search (a `no_std` engine, for example `regex-automata` without `std`, recorded in THIRD_PARTY.md) and replace, search across a directory through the program's VFS scope;
  2. column selection, indentation of a selection, comments, brackets, auto-indent, line operations, multiple cursors;
  3. large files by window; the hex mode;
  4. sessions, backups and recovery (a journal of edits in `data/apps/edit/`), external changes noticed by the file's size and time;
  5. the system clipboard (a request to `KRN`: a clipboard service with a capability per program, text only, the user's paste explicit);
  6. settings and key bindings; macros;
  7. the outline and jumps; spell checking.
- **Budgets:** the editor's memory is its own quota (MC-5.1); a file too large for it opens by window, never fails silently.

## Acceptance criteria

- **Host tests** for each feature's core: regular expressions against a stated suite, column operations, multiple cursors, the journal's recovery after a cut at every edit.
- **The QEMU `edit` suite** grows with each step: two files in a split view, a regular-expression replace, a 64 MiB file opened and edited, a recovery after the program is killed, the clipboard between `edit` and the shell.
- **Docs:** section 4.2 of `docs/tools/README.md` and `README_RU.md`, the help line and `edit --help`.

## Related

[067](../issues-done/067-editor.done) (the editor), [000-APP-0030](../issues-done/000-APP-0030-syntax-colours-in-edit.done) (syntax colours), [258](258-wysiwyg-document-editor.md) (the WYSIWYG editor).
