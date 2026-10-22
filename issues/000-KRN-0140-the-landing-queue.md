# 000-KRN-0140 — The landing queue: one integrator gates the candidate that becomes `main`

**Type:** process, scripts · **Owner:** kernel session (the orchestration track) · **Priority:** P1 · **Status:** in use (2026-10-21): rounds 1, 2, 3 and 5 landed, round 4 returned · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.1, MC-12.9 (evidence names its configuration: `main`'s exact tree is gated)

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
   - A request that conflicts in code is returned at once. A conflict in text only the integrator resolves, keeping both sides (`resolve`, recorded by git's `rerere`).
3. **One gate on the candidate.**
   - Every group of `scripts/ci_local.sh --list` runs, spread over the Effector machines.
   - A group that failed runs once more, alone.
4. **The result.**
   - **Green:** `main` fast-forwards to the candidate, and every request in it has landed in one round.
   - **Red:** the request that broke it is returned, and the others wait for the next round.
5. **The answer:** one file in `LANDING/` names each request landed, returned or superseded.

**The tools:**
- **`scripts/land.sh`** (an entry to `scripts/land.py`) with `queue`, `candidate`, `resolve`, `gate`, `push` and `announce`.
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
- [x] A round with requests from two tracks lands both at once (round 2: four tracks).

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

**The second round, 2026-10-21.** Its first `candidate` returned LNK's request for a conflict in one line of `issues/README.md` (the list of requests, changed on both sides). Such a conflict in an index file would come back in most rounds, and returning it costs the track a whole round.
- **Now** a conflict in text only stays in the round for the integrator: `resolve REQUEST` opens it, `resolve --done REQUEST` records the resolution with git's `rerere`, and the next `candidate` reuses it.
- **A conflict in code** is still returned at once.
- `tests/land_test.py` covers both.

**The second round landed.** Gate `land-97c4a4c1` on candidate 97c4a4c: every group green, on i9, un-rizen (two lanes) and PCU, preset `full`, with no group run again.
- **Its four requests:** LNK (`claude/LNK-link` eeacc4a), DRV (`claude/550-DRV-0036-radio-counts` b468541), KRN (fb89097, the script above) and APP (`claude/wizardly-franklin-kec1a9` 893ef48).
- **Two text conflicts,** both in `issues/README.md`, were resolved by hand, keeping both sides:
  - LNK's merge, recorded with `resolve` and reused by the next `candidate`;
  - `main`'s later text, by `push`'s hand merge.
- **`main` = 1d84162.** The answer is `LANDING/2026-10-21T1350Z-KRN-landed-97c4a4c1.md`.

**Rounds 3 to 5 (2026-10-21):**
- **Round 3** landed EFF and KRN: `main` e4fd3b3.
  - KRN's first request changed the same aarch64 group line as EFF's. That conflict in code would have returned EFF's request, so KRN posted a narrower request.
- **Round 4** (KRN alone) was returned. The host tests failed twice: the IDL fuzzer must list every generated type, and `init.wit` 1.5's two new types were missing.
- **Round 5** landed DRV, LNK and KRN, with ABI 14: `main` 3bdd026.
- **A container restart** stopped the gate script with one group left running on i9.
  - `gate` run again with the same tag found every group's operation by its idempotency key and read the results back from Effector, the last one completed meanwhile.
  - It was green within a minute, and no group ran twice.
  - The keys are `land-<tag>-<lane>-<n>-<hash of the command>`, so a gate can be taken up again after any interruption.

**Round 6 (2026-10-21/22)** landed KRN alone: 176-KRN-0147, 850-KRN-0145 and 850-KRN-0146.
- The candidate was the branch itself, a fast-forward of `main`.
- Gate `land-b05bfb22`: every group green, none run again, in 1 h 35 min on i9, un-rizen (two lanes) and PCU.
- **`main` = b05bfb2.** The answer is `LANDING/2026-10-22T0043Z-KRN-landed-b05bfb22.md`.
- GitHub CI on b05bfb2 was still queued: its runs finish about a day after their push.

**Round 7 (2026-10-22)** returned APP's request (`claude/wizardly-franklin-kec1a9` df433ce, time zones).
- **Gate `land-803dd2f8`** on 803dd2f, on `main` 4bb89de: red.
- **The cause:** `services_suite` still expects the old `date -h` line, and APP's shell changed it. It failed in "x86: services, storage, audio" and "x86: one CPU", twice each, the second time alone.
- **Two first-run failures on i9** passed alone, so they were not the request's:
  - in "x86: boot, display, network…", `download`'s read-back stopped after 16 of 30 MiB;
  - "x86: files and block writes" ran past its hour, and its log was lost with the operation.

**Round 8 (2026-10-22)** landed all three requests: LNK's COMMAND (`claude/LNK-link` 97d48a3), NET's parser sessions and `BADGE_LAUNCH` (`claude/relaxed-meitner-5bmhpz` a1488f4) and KRN's ABI 15 and `init.wit` 1.6 (`claude/youthful-mendel-mf1soy` 1ea50b9).
- **Gate `land-aa50204e`** on candidate aa50204, on `main` 28202e1: every group green, none run again, in 1 h 43 min on i9, un-rizen (two lanes) and PCU.
- **`main` = aa50204.** The answer is `LANDING/2026-10-22T0729Z-KRN-landed-aa50204e.md`.

**Round 9 (2026-10-22)** landed KRN's request (`claude/youthful-mendel-mf1soy` f835b53d): 351-KRN-0151 and 800-KRN-0142.
- **Gate `land-f8d76dd9`** on candidate f8d76dd, on `main` c930067: every group green, none run again, in 1 h 38 min on i9, un-rizen (two lanes) and PCU.
- **`main` = f8d76dd.** The answer is `LANDING/2026-10-22T0921Z-KRN-landed-f8d76dd9.md`.
- The builds took 2 to 5 s, against minutes in round 8: the machines' trees had round 8 built, and only `init` and `loader` changed. The new checks of `slot_applications_check` and the `link` suite passed in the gate; they fail on the old `init` and `loader`.
- **The cost of carrying the applications.** "aarch64: files, network and TLS" took 1810 s, against 1436 s in round 8. "x86: files and block writes" took 1141 s, against 896 s.
- **Mis-stamped messages.** KRN's request and messages of this round were named 0812Z, 0814Z and 0820Z. They were written at about 07:40 UTC.

**Round 10 (2026-10-22)** landed KRN's request (`claude/youthful-mendel-mf1soy` 0ab4ca90), 550-NET-0041, made by the kernel session as NET's owner.
- **Gate `land-0ab4ca90`** on 0ab4ca90, a fast-forward of `main` c045eaa: every group green, none run again, in 1 h 43 min.
- **`main` = 0ab4ca9.** The answer is `LANDING/2026-10-22T1312Z-KRN-landed-0ab4ca90.md`.

  The cause is not established. If it recurs, it gets an issue of its own.
- **The gate script was stopped** at its 2-hour background limit, with two i9 groups left. Run again with the same tag, it took up every finished operation by its key, ran the rest and the failed groups alone, and called it red.
- **The answer** is `LANDING/2026-10-22T0346Z-KRN-round-803dd2f8.md`, with the details in APP's directory.

## Related

AGENTS.md section 4; [docs/effector.md](../docs/effector.md) sections 1 and 4; [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md) (its gate g0142, still run the old way).
