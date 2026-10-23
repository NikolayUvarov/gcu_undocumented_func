# 800-KRN-0155 — The link's console: a word of `data/mode.conf`, counted only with `dev`

**Type:** `init`, IDL (`init.wit` 1.8) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — (for 800-LNK-0019) · **Constitution:** MC-3.11, MC-10.2, MC-12.6

From LNK's request of 2026-10-23 (`KRN/2026-10-23T0223Z-LNK-a-mode-word-link-console-in-init.md` on `internal-dev-messages`), after the maintainer's word on the MacBook Pro's rewrite: "effector and effector485 have to have equal rights in maintenance mode, or we need an additional mode line for the 485 debug link".

## Problem

- **What failed.** The owner's step `msh /data/link/effector/start.msh`, sent over the cable, was refused: the shell runs no script from the link (800-APP-0102).
- **What is missing.** A way for the owner to say, on the disk, that the cable's console may do what the local one does.
- **Whose word it is.** The owner's, written when the disk is written (MC-12.6), not the cable's own (MC-3.11).

## Plan

1. **The word.** `init` takes `link-console` in `data/mode.conf`, beside `development` and `maintenance`.
   - It counts only when `link.conf` has `dev` (800-KRN-0139).
   - Without `dev` it is left out and logged: `[INIT] data/mode.conf: WORD LEFT OUT: link-console (NO dev IN link.conf)`.
2. **The boot line names it,** for example `[INIT] MODE: MAINTENANCE AND LINK CONSOLE (data/mode.conf)`.
3. **The interface: `init.wit` 1.8,** a minor version.
   - `link-console: func() -> result<bool, error>` at the end; any client may ask.
   - `mode-state` stays as it is: a new field would change its layout, a major version (docs/idl).
4. **`set-mode` keeps the word** when it writes the file again.
   - It does not turn the word on or off: only the disk's file does, so the owner decides it before the machine boots.
5. **No new grant here.**
   - The shell decides what its console from the link may do when the word is on (APP).
   - `link` passes lines as before (LNK).

## Acceptance criteria

- [ ] The `modes` suite (x86 and aarch64):
  - without `dev`, the word is left out with its line, and `link-console` answers false;
  - with `dev` in `link.conf`, the boot line names the link's console, and `link-console` answers true.
- [ ] `set-mode` keeps the word in the file.
- [ ] `docs/idl` lists `init.wit` 1.8.

## Related

800-LNK-0019 (on `claude/LNK-link`), [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md), [750-KRN-0143](750-KRN-0143-init-follows-the-development-and-maintenance-modes.md), [800-APP-0102](800-APP-0102-the-shells-run-for-the-links-client.md).
