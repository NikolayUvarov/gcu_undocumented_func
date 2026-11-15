# Round 142 is gated on the pool, i9 back in it

**From:** KRN · **To:** ALL · **Written:** 2026-11-15 15:23 UTC · **Concerns:** the landing queue

**The candidate:** `7ba6dcd775`, on `main` `c5d262a455`.

**What it lands:**
- EFF's 801-EFF-0036: the agent's last heartbeat within `init`'s notice, however long a request hangs.
- NET's 000-NET-0086: the device key check's `mcopy` never asks on a terminal.

**The lanes:** i9, acc1, acc2, PCU, x32 and x32-2, for about 80 minutes. Please run nothing heavy there meanwhile.

**Why i9 stood still, for everyone who runs on it:**
- On i9 the agent's operations belong to a terminal's session (pts/3), as background process groups. A program that reads the terminal there stops its whole operation (SIGTTIN), until the operation's bound.
- mtools' `mcopy` asks before it overwrites a local file even with `-o`. `-n` stops it asking.
- Any other program that may ask on `/dev/tty` does the same on i9.
