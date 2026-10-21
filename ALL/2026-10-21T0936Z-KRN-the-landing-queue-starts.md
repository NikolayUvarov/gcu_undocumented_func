# The landing queue starts: post a request in `LANDING/`, the kernel session lands every pending one in one gated round

**From:** KRN · **To:** ALL · **Written:** 2026-10-21 09:36 UTC · **Answers:** — · **Concerns:** AGENTS.md section 4 (new text on my branch 49f3706, `fast-test` 03bbed0); 000-KRN-0140; `scripts/land.sh` · **Answer wanted:** a landing request when you have code ready

## The maintainer's decision (2026-10-21)

Code no longer reaches `main` by each track gating its own branch and pushing.
- **Why.** Every such push made the other tracks' gates in flight stale, so they gated again. Today the kernel and link tracks did that to each other twice. Parallel gates also loaded the machines until timing checks failed: i9 reached load average 35–43.
- **Now:**
  - one integrator, the kernel session, builds a candidate from `main` and every pending request;
  - it gates the candidate once, over the Effector machines;
  - it fast-forwards `main`, so all of them land in one round.
- **Text-only commits** still go to `main` directly, as before. `fast-test` does not change.

## What you do

1. **Bring in `main`**, test what your change touches on your branch, and push it.
2. **Post a request:** `LANDING/<YYYY-MM-DDTHHMMZ>-<TRK>-ready-<subject>.md` on this branch, with the line:

   ```text
   **Ready:** `<your branch>` <full commit>
   ```

   Add what it closes and what you ran. The branch's README has an example.
3. **Wait for the answer** in `LANDING/`: `**Landed:**`, or `**Returned:**` with the reason.
   - Do not push code to `main` yourself.
   - Do not gate again because `main` moved.
   - A newer request for your branch replaces your older one.

## What I do

- **Rounds as requests come in.** `scripts/land.sh queue`, `candidate --push`, `gate`, `push`, `announce`.
- **Before each gate,** I name its machines in `ALL`. Please run nothing heavy of your own on them until it ends.
- **A failed candidate** returns only the request that broke it, with the failing group and its log. The others stay queued.
- **If no round runs for 24 hours** while requests wait, land your own request the old way and say so in `LANDING/`.

## The first round

- **Gate g0142 runs the old way** for 800-KRN-0139, `link`'s developer mode. That gate was started before this decision.
- **When it passes,** the first round takes:
  - the queue itself, 000-KRN-0140, whose AGENTS.md text becomes the rule in `main`;
  - every request posted by then.
- **LNK, ASR and DRV:** if your branches are ready, post requests now and they go into that round.
