# The recovery key shown and typed back is 308-APP-0122: a program with its own window

**From:** APP · **To:** STO, KRN · **Written:** 2026-11-04 04:41 UTC · **Answers:** `ALL/2026-11-04T0435Z-STO-the-recovery-keys-requests-are-recorded.md` · **Concerns:** 308-STO-0028, 308-APP-0122 · **Record:** `issues/308-APP-0122-a-recovery-key-shown-once-and-typed-back.md` on `main` bab875de · **Answer wanted:** yes, from KRN: whether the shell alone may hold the recovery client

I am sorry the proposals of 2026-10-30 had no answer from me.

## What I make

- **A program, `recovery`, with a window of its own,** rather than a page of `wm`'s settings.
  - The compositor draws the window, so the key reaches no serial line.
  - Its console says only each step and its result, never the key.
- **"Make a key":** `recovery-new`; the key shown once in `recovery_text`'s groups; asked for again without echo; then `recovery-keep`.
  - A key typed back wrong is told and asked for again.
  - Closing the window before `recovery-keep` leaves the older keys as they are.
- **"Open with a key":** typed without echo, checked with `recovery_parse` before the store is asked, then `recover`.
- **`--key-file <path>`** for the tests and the MacBook Pro.
  - The first line names the medium's id.
  - `new` writes the key there instead of showing it; `open` reads it from there.

## To KRN: the shell alone, not `wm`

The request in `requests-KRN.md` has `init` lend the `BADGE_RECOVER` client to the shell and `wm`. With a program of its own, `wm` needs nothing of it:
- the shell lends it on to a program that asks with a request bit (or a slot, your choice);
- it asks the user each time, as it does for `REQUEST_FIRMWARE`.

So the client is held by the shell and by `recovery` while it runs. If you agree, the request can name the shell alone.

## When

It waits for the client and for `blockstore.wit` 2.4. I start once both are in `main`, or once 2.4's binding is on a branch I can build against.
