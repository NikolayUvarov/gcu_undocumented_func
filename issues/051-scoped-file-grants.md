# 051 — Scoped file grants for launched programs

**Type:** security/architecture · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** track G, tools plan F7 (powerbox) · **Constitution:** MC-3.4, MC-3.7, MC-3.11

## Problem

A program that asks for a file (`REQUEST_FILE`, the editor) gets the shell's own VFS client in `SLOT_FILE` (047): it may change any file on `ram:` and in `data/`, not only the one the user named. The plan (docs/tools F7) is that `edit notes.txt` gets that file and its directory, nothing else. VFS handles are bound to the PID and badge of the client that opened them, so the shell cannot pass a handle it opened, and a badged capability cannot be badged again.

## Plan

- `vfs.wit`: `scope(dir, writable) -> endpoint` — `vfs_server` mints a client capability from its own endpoint with a fresh badge bound to that directory (never wider than the caller's handle, MC-3.4); paths through it are relative to the directory and `..` is refused as today. Scopes are counted per client (MC-10.2 quota) and dropped when the holder exits or the granting client closes the directory.
- Shell: for `REQUEST_FILE` it opens the directory of the named file (the user's zone decides read-write or read-only) and lends the scoped client instead of its own.
- `edit`: unchanged (it already uses whatever client is in `SLOT_FILE`); `fm` F4 (048) lends the same.
- `.mind_request` gains `dir:rw` for tools that need a directory (fm's second panel).

## Acceptance criteria

- `edit data/a.txt` saves `data/a.txt` (through `a.txt.tmp`), but through its client cannot open `ram:`, other directories of `data/` or anything above; the isolation suite checks it with a test program that asks for a file.
- A scoped client stops working after the program exits (its badge is revoked).

## Related

[047](../issues-done/047-editor.done), [docs/tools](../docs/tools/README.md) F7, F8.
