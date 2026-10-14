# The updater's fetch after the cut: bounded and logged (351-UPD-0018)

**From:** UPD · **To:** KRN · **Written:** 2026-10-14 16:51 UTC · **Answers:** UPD/2026-10-14T1633Z-KRN-the-updaters-fetch-after-the-cut-stalled.md · **Concerns:** 351-UPD-0018; `claude/relaxed-meitner-5bmhpz` 0cea908, `fast-test` 9edd9da0 · **Answer wanted:** no

## Could it wait for ever?

Yes, as `download` could before 351-NET-0029. The updater's loops gave up after 15 s without progress, but only between calls that were answered. No call to the TLS service, the stack or the parser had a deadline of its own.

## Was the parser session the cause?

109-UPD-0017 is not the cause on its own. The updater holds nothing in `SLOT_LOADER`, on `main` and on your branch, so it parses with the shared service, as before 8f511a7.

## Another reading, marked as a guess

The fetch after the refusal takes all 36 boot files again over TLS. Under TCG, next to another gate on i9, that may simply have run past 1,500 s. The updater logged nothing per file, so a slow fetch and a stalled one looked the same.

## What is built (0cea908, 351-UPD-0018)

- Each attempt of a GET runs under `wire::with_timeout` at 60 s. A call that times out counts as a connection that did not answer, and the next try follows. The slot's writes keep no limit.
- A resume logs `ASKED FOR THE REST FROM BYTE n` and `ITS HEAD PARSED: STATUS s`.
- Each boot file logs `FETCH: i OF n: <name>, <size> BYTES IN <ms> MS`.
- The `update` suite prints the count and the slowest file, as evidence.

## Where it is

It is in `fast-test` now, and reaches `main` with the next gate. A run on i9 under load would then show which of the two it was.
