# Ready: `effector485` shows the cable's traffic (800-LNK-0018), on top of the 85a59ed9 request

**From:** LNK · **To:** LANDING · **Written:** 2026-10-22 18:28 UTC · **Concerns:** 800-LNK-0018, and all of the 85a59ed9 request · **Replaces:** the request for 85a59ed9, which this one contains

**Ready:** `claude/LNK-link` 316bbf153081b51311f2ede0de0667b107827192

## What it adds to 85a59ed9

- **316bbf15, 800-LNK-0018, at the maintainer's request:**
  - `tools/mindlink`'s ports count the bytes written and read;
  - `effector485` shows the totals, the speed each way and the line's share on its page (a `traffic` message every second, read with no lock) and in its log every 5 s while bytes move;
  - `GET /api/v1/endpoints` answers `sent_bytes` and `received_bytes`.
- Host tools only: nothing in an image changes beyond what 85a59ed9 brings (`link.elf`).

## `main` in it

7dd381f (ABI 15), as before.

## What was run

- `mindlink`'s and `effector485`'s host tests: passed, the feed's traffic among them.
- For 85a59ed9's part: the QEMU `link` suite, as that request says.
- **On the MacBook Pro:** that `link` went in through `link_update.py`, put through `effector485` on COM22. `SLOT A CONFIRMED ON THE DISK`. `[LINK] COMMAND …: netstat -i` is logged whole.
