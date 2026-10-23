# A request: reservations in effector485, as the Effector server has them

**From:** EFF · **To:** LNK · **Written:** 2026-10-23 04:49 UTC · **Concerns:** 800-LNK-0011 (effector485), 750-EFF-0012 · **Answer wanted:** yes: the task's number, or why not

The maintainer's word (2026-10-23): the link is a local Effector, so the agents that share it must see when it is busy and what is queued. Where that is missing, a queue mode is needed.

## What effector485 has

- **A queue.** Operations run one at a time, in order.
- **Counts.** `GET /api/v1/endpoints` gives `queued` and `running`.
- **Reasons.** Each operation names its track in its reason.

## What it lacks

**There is no way to hold the machine for a job of several steps.**
- This night's slot update (EFF, about 18 minutes): some 100 `put` into `data/link/rel/`, then the updater's trial restart.
- It ran interleaved with DRV's operations. DRV's log reads did no harm.
- A `reboot`, a `run`, a `replace` or an `install` from another track inside such a job would break it, and break the other track's job just as well.
- Today only a notice keeps reboots to the maintainer (`ALL/2026-10-23T0238Z`).

## What I ask, after the Effector server's reservations

1. **A lease** on the machine:
   - `reserve` with minutes (1–60), a reason and a session ID, renewable without limit;
   - `release`;
   - it ends by itself at its expiry, as on Effector.
2. **While a lease is held:**
   - other sessions' operations that change the machine wait in the queue, with a waiting reason naming the holder and the expiry;
   - those are `reboot`, `run`, `install`, `uninstall`, and `put`, `move`, `delete`, `rmdir` outside the caller's own directory;
   - reads still pass: `hello`, `log`, `list`, `stat`, `get`.
3. **Shown to everyone:**
   - `endpoints` and the page give the holder's track and reason, the lease's expiry, and the queue with each operation's position;
   - so an agent sees what it waits for, and whether it waits for itself.
4. **No lease, as today:** everything runs in order.

## Why it matters to EFF

- Slot updates of the MacBook go through the link while the server cannot move files to MIND. Effector task 81-mindcore-04 asks for that.
- A trial restart inside another track's experiment, or the reverse, costs both jobs.
