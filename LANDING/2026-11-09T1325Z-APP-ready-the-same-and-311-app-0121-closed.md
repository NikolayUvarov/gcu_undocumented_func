# Ready: the same as 1319Z, and 311-APP-0121 closed (a write through the read-only store scope refused)

**From:** APP · **To:** LANDING · **Written:** 2026-11-09 13:25 UTC · **Concerns:** 000-APP-0126 (it advances), 000-APP-0123, 262-APP-0112 and 311-APP-0121 (they close), 000-APP-0120's follow-up · **Replaces:** `2026-11-09T1319Z-APP-ready-the-same-with-the-shells-other-receiving-slot-moved.md`, not answered yet

**Ready:** `claude/wizardly-franklin-kec1a9` ff268f520df3745982af9d0a38ee0f75b5eee2b6

It contains `main` 39351fa0f.

## What changed since 1319Z

Everything 1319Z brought, and:
- **07863d3bd and after:** `tests/storewrite_stub`, a test-only console program that asks for no file, built by `scripts/x86_fixtures.sh`.
  - The `shell` suite's `storefs_check` runs it on `notes:docs/z.txt`.
  - Through the read-only scope the shell lends it, the listing answers and the write is refused.
  - 311-APP-0121 is closed.
- **No code of the shell changed.**

## Tested on this container's QEMU x86, 4 CPUs, TCG

- **The `shell` suite with it:** 15 checks passed. The stub printed `STOREWRITE READ notes:docs: 1 ENTRIES` and `STOREWRITE WRITE notes:docs/z.txt REFUSED: Denied`, and `ls` found no `z.txt`.
- **The other runs:** as in 1319Z.
- **Not run here:** aarch64, and the other groups.
