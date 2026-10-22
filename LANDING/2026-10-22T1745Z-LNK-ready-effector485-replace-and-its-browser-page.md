# Ready: `effector485`'s kind `replace` (800-LNK-0016) and its page in a browser (800-LNK-0017)

**From:** LNK · **To:** LANDING · **Written:** 2026-10-22 17:45 UTC · **Concerns:** 800-LNK-0016, 800-LNK-0017 · **Replaces:** the 14:12 UTC request (f477bf8), which this one contains

**Ready:** `claude/LNK-link` 5da81a11cbbc30d834a7cffb93f836e743c641b5

## What it brings

- **Everything of f477bf8** (the 14:12 UTC request): `link` runs the shell's lines, and REPLACE.
- **883c50a9, 800-LNK-0016:** `effector485`'s kind `replace`, so a service is replaced through the shared daemon. EFF's daemon on COM22 runs it since 17:02 UTC.
- **5da81a11, 800-LNK-0017, at the maintainer's request:**
  - `effector485` serves a page at `/`: the cable's state, the operations and an operation's output, live over a WebSocket (`/api/v1/stream`), with buttons for the usual operations;
  - the feed never waits for the daemon's lock and drops a browser that stops reading, so the cable's work is not held up;
  - requests from another site (`Origin`) or another host name (`Host`) on a loopback address are refused;
  - the worker no longer marks a change, and rewrites the state file, on every idle turn.
- Only `tools/mindlink/` (the `effector485` binary), `docs/link/effector485*.md` and `issues/` change after f477bf8. Nothing that goes into an image.

## `main` in it

7dd381f (ABI 15), as in f477bf8.

## What was run

On the link track's WSL machine, x86:
- `mindlink`'s and `effector485`'s host tests: all passed, `the_page_its_feed_and_other_sites` among them;
- the page in a headless Chromium against `mindlink serve`: light and dark, 1400 and 420 pixels wide;
- for f477bf8's part, the QEMU `link` suite as that request says. This part does not touch what the suite runs.
