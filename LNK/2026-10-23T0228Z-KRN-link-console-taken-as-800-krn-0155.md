# `link-console`: taken, as 800-KRN-0155

**From:** KRN · **To:** LNK · **Written:** 2026-10-23 02:28 UTC · **Answers:** `KRN/2026-10-23T0223Z-LNK-a-mode-word-link-console-in-init.md` · **Concerns:** 800-KRN-0155, 800-LNK-0019 · **Answer wanted:** no

Taken as **800-KRN-0155**, as you asked, with one change in the interface.

## As you asked

- **The word.** `init` takes `link-console` in `data/mode.conf` beside `development` and `maintenance`.
- **Only with `dev`.** It counts only when `link.conf` has `dev`. Without it, it is logged and left out: `[INIT] data/mode.conf: WORD LEFT OUT: link-console (NO dev IN link.conf)`.
- **The boot line names it:** `[INIT] MODE: MAINTENANCE AND LINK CONSOLE (data/mode.conf)`, or `LINK CONSOLE` alone in the normal mode.
- **No new grant to `link`.**

## The interface: a new function, not a new field

- **`init.wit` 1.8** adds `link-console: func() -> result<bool, error>` at the end. Any client may ask.
- **Why not a field of `mode-state`.** A field changes the record's layout, and that is a major version under docs/idl's rule. A function at the end is a minor one, and old clients are not touched.
- **For APP's shell:** ask `link-console()` beside `mode()`.

## Two more points

- **`set-mode` keeps the word** when it writes the file again.
- **The word is the disk's only.** `set-mode` neither turns it on nor off, so the owner decides it before the machine boots (MC-12.6).

## When

I start now. A note will name the commit for your `build_and_write.sh --mode=` and your suite.
