# Ready: 800-LNK-0033 and 800-LNK-0034, the cable's lost answers and a stale log line; 800-LNK-0032 closed on the Mac (replaces the request of 07:52)

**From:** LNK · **To:** LANDING · **Written:** 2026-10-31 09:35 UTC · **Concerns:** 800-LNK-0033 and 800-LNK-0034 (advanced), 800-LNK-0032 (closes), 800-LNK-0035 (new) · **Replaces:** `LANDING/2026-10-31T0752Z-LNK-ready-a-busy-machine-ends-no-put.md` (8ddc98ff), if not yet answered

**Ready:** `claude/LNK-link` 46ef7500d6a835680053ca9c14df0e488b2e5194

## What it brings

On `main` 1c68b668, merged. The earlier request's commits are included.

- **a454c709, 800-LNK-0033:** `linkproto`'s tries wait twice as long as the one before, up to 4 s.
- **ff9d0cf8, 800-LNK-0034:** `scripts/link_update.py` takes no old log line for a new one: without its anchor it goes by the uptimes. It has two host tests in `tests/link_update_test.py`.
  - **Why:** the MacBook Pro's update ended `exit 1` on an old `REFUSED: Older` though the Mac confirmed the slot.
- **The same commit closes 800-LNK-0032 on the Mac and splits the pointer off as 800-LNK-0035.**
- **92bf4112** carries only the rename of 800-LNK-0032: its `git add` failed. ff9d0cf8 carries what its message names.

## Tested, on LEG9 (WSL2, Ubuntu 24.04)

- **Host tests:** `linkproto` (24), `tools/mindlink`, `tests/link_update_test.py`, before and after merging `main`.
- **CI groups through `scripts/ci_local.sh`,** on ff9d0cf8's tree before `main` was merged: "x86: the updater and an update through the link" PASS (691 s), and the QEMU `link` suite on x86 PASS.
- **On the hardware, through the new `effector485`:**
  - the DEXP: 24 puts of 8.4 MB, none lost;
  - the MacBook Pro's slot update: 107 puts, none lost; release 202610310825 confirmed in slot B;
  - Ctrl+Alt+F6 and `keys` over the cable on the Mac (800-LNK-0032's criterion).
- **Not run on this exact tree:** the QEMU suites after `main` 1c68b668 was merged. That merge brought STO's and KRN's code, none of it in `linkproto` or `link_update.py`.
