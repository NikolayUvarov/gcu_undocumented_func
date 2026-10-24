# 800-LNK-0022 — effector485 hung after a cancel: `save` pruned inside a route, and the poisoned lock stopped every request

**Type:** bug · **Owner:** `LNK` · **Priority:** P0 · **Status:** in progress (2026-10-24) · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-10.2, MC-12.1

## Problem

- **What was seen.** Twice, `effector485` stopped answering: on 2026-10-23 at 06:14 UTC (LNK) and on 2026-10-24 at about 07:50 UTC (EFF).
  - Every request got a connection closed with no answer: "the daemon's answer has no head", or `curl` exit 52.
  - The window showed every request thread panicking at `main.rs:189`, `state.lock().unwrap()`, on a `PoisonError`.
- **The cause** (found by EFF, `LNK/2026-10-24T0753Z-EFF-the-hang-is-a-panic-save-prunes-then-view-indexes.md`):
  1. `POST /api/v1/operations/cancel` took the operation's index from `cancel`, and a queued operation is finished on the spot.
  2. With more than `ENDED_KEPT` (500) ended operations, `save()` dropped the oldest, which moved every later index down.
  3. `view(i)` then read past the end and panicked while the route held the state's mutex. The mutex was poisoned, and every later `lock().unwrap()` panicked in turn.
- **Both hangs fit.** Both came right after a cancel of a queued operation, the daemon's own operation waiting behind its own lease (800-LNK-0021): the lease's variable had not reached the Windows program from WSL.

## Plan and what was done

1. **Prune only where no index is held.** `State::prune()` is split from `save()`. The worker calls it at the end of each round, between operations.
2. **Answers are built before the save:** for a submit and for a cancel.
3. **A poisoned lock is taken back** (`worker::lock`: `unwrap_or_else(into_inner)`), in the routes, the worker and the page's feed. One request that panics then fails alone, and the daemon goes on.
4. **The docs** (`docs/link/effector485.md`, RU) say that from WSL the lease goes as `--reservation`, or with `WSLENV`.
5. **Tests** (`main.rs`):
   - `a_cancel_at_the_ended_limit`: 501 ended operations and a queued one; the cancel answers 200 for that operation, and the endpoints still answer;
   - `a_poisoned_lock_is_taken_back`.

## Acceptance criteria

- [x] The host tests above pass, with the rest of `tools/mindlink`'s.
- [ ] The daemon on the maintainer's Windows machine runs this build, and a cancel of a queued operation under a lease answers.

## Related

[800-LNK-0011](800-LNK-0011-effector485.md), [800-LNK-0021](800-LNK-0021-effector485-reservations.md).
