# 000-KRN-0140 — The landing queue: one integrator gates the candidate that becomes `main`

**Type:** process, scripts · **Owner:** kernel session (the orchestration track) · **Priority:** P1 · **Status:** in use (2026-10-21): the first round landed; a round with two tracks' requests is left · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.1, MC-12.9 (evidence names its configuration: `main`'s exact tree is gated)

## Problem

The maintainer asked on 2026-10-21 why the parallel tracks keep gating again, and how to stop it.
- **The rule until now.** AGENTS.md section 4: each track gates its branch merged with the current `main`, for 1.5–2 hours on four machines, then fast-forwards `main`.
- **What it caused:**
  - **Stale gates.** The first track to push made every other gate in flight stale, and each had to merge and gate again. One track lands per round, and a track can lose many rounds in a row. On 2026-10-20 and 21 the kernel and link tracks re-gated each other three times: g0132 → g0138, g0140 → g0141.
  - **Shared machines.** The parallel gates loaded the same machines until their timing checks failed: load average 35–43 on i9 while another track's gate ran, and four of g0140's groups failed for it.
- **What stays.** The gate itself is right: the exact tree that becomes `main` is tested, and evidence is not carried over from another tree (MC-12.1, MC-12.9).

## Plan

**The landing queue** (merge queue, as in bors or Zuul), at the maintainer's decision of 2026-10-21:

1. **A track posts a landing request.** This is a message `LANDING/<time>-<TRK>-ready-<subject>.md` on `internal-dev-messages`, with ``**Ready:** `<branch>` <commit>``.
2. **The integrator builds a candidate.** The kernel session takes `main` and merges in every pending request, in order.
   - Merges only: commits and authorship stay.
   - A request that conflicts is returned at once.
3. **One gate on the candidate.**
   - Every group of `scripts/ci_local.sh --list` runs, spread over the Effector machines.
   - A group that failed runs once more, alone.
4. **The result.**
   - **Green:** `main` fast-forwards to the candidate, and every request in it has landed in one round.
   - **Red:** the request that broke it is returned, and the others wait for the next round.
5. **The answer:** one file in `LANDING/` names each request landed, returned or superseded.

**The tools:**
- **`scripts/land.sh`** (an entry to `scripts/land.py`) with `queue`, `candidate`, `gate`, `push` and `announce`.
  - The gate's presets are the measured splits of g0141 (`full`) and g0142 (`no-i9`).
  - The Effector key is read only from `$EFFECTOR_KEY_FILE` and sent only in the `X-API-Key` header.
  - `push` merges a text-only move of `main` without another gate, and refuses when `main` moved with code.
- **`tests/land_test.py`**, in `scripts/host_tests.sh`. It covers:
  - requests, answers and superseding;
  - the text-only rule;
  - the gate's plan placing every group of `ci_local.sh --list` once, after its lane's builds;
  - the verdict;
  - a candidate built, refused without a gate, pushed over a text-only move of `main`, refused over a code move, and announced, on throwaway repositories.
- **AGENTS.md section 4** (and sections 3, 5, 6 and 7), **CONTRIBUTING.md**, and **docs/effector.md** section 4 in English and Russian.

**Kept as before:**
- text-only commits go to `main` directly;
- `fast-test`, `ci_watch.sh` and pull requests;
- if no round has run for 24 hours while requests wait, a track may land itself as before and say so.

## Acceptance criteria

- [x] `tests/land_test.py` passes in the host tests.
- [x] The first landing round: a candidate with this task, gated by `scripts/land.sh gate`, pushed and announced (below).
- [ ] A round with requests from two tracks lands both at once.

## Progress

**The first round, 2026-10-21.** It landed this task's own request.
- **The candidate.** `claude/youthful-mendel-mf1soy` 48ff4b2, a fast-forward over `main` 98586f1.
  - The first gate, `land-82debc67` on 82debc6, was stopped: i9's x86 build gave no result, since `ci_local.sh` refuses the host part where no sshd is, and the lane went on with an old image.
  - 48ff4b2 fixed both (the host part's builds run where sshd is absent; a failed build stops its lane), with a request that superseded the first.
- **The gate, `land-48ff4b28`:** every group of `ci_local.sh --list` on i9, un-rizen (two lanes) and PCU, preset `full`: green.
- **The push.** `main` had moved with text (4e416cd: 750-UPD-0019, 800-NET-0038), and its merge conflicted in `issues/README.md`.
  - The conflict was resolved by hand, keeping both rows. The merge changes `issues/` only.
  - `main` = a67ca61. The answer is `LANDING/2026-10-21T1211Z-KRN-landed-48ff4b28.md`.
- **What it changed in the script.** `push` stopped with git's error on that conflict. It now leaves the merge in the candidate's tree to be resolved by hand, and on the next run pushes the hand merge only if it adds text alone (the next commit; `tests/land_test.py` covers it).

## Related

AGENTS.md section 4; [docs/effector.md](../docs/effector.md) sections 1 and 4; [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md) (its gate g0142, still run the old way).
