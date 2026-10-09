# Contributing with coding agents

MIND Core is developed by several coding agents working in parallel, each in its own **track**, under the rules below. Other people are welcome to join with their own agents (Claude Code, Codex, Cursor, Aider or any other) or by hand. This file says how to join and what an agent must do so that its work stays within the project's [Constitution](constitution/EN/MIND_CORE_Constitution_v1.6.md) and does not collide with the other tracks. It adds to [CONTRIBUTING.md](CONTRIBUTING.md), and the rules there still hold.

Agents read this file. Point yours at it at the start of every session (section 7 has a brief you can paste).

## 1. What governs the work

From strongest to weakest:

1. **The Constitution:** [constitution/EN](constitution/EN/MIND_CORE_Constitution_v1.6.md), with a Russian version in `constitution/RU`. Only its articles are normative. Clauses are cited as `MC-<article>.<clause>`.
2. **Platform profiles:** [docs/profile](docs/profile/README.md). They say which requirements the code meets, on which platform, with what evidence and what trusted computing base (TCB).
3. **The roadmap:** [ROADMAP.md](ROADMAP.md) and [ROADMAP_RU.md](ROADMAP_RU.md). They set the order of work and the tracks.
4. **Issues:** [issues/](issues/README.md). An issue is one task with its acceptance criteria.

What follows from this order for an agent:

- **Every task serves a cited clause or roadmap item.** An issue names the `MC-…` clauses or the roadmap item it serves.
- **Intentions are not guarantees.** Never present a plan or design intention as an existing guarantee (MC-12.3). A profile lists what is not done as well as what is.
- **Tests are not proofs.** Never call test results a proof (MC-12.2). Never carry evidence from one configuration over to another (MC-12.1, 12.9).
- **No redefining requirements.** Never weaken or reinterpret a requirement to make the code conform (MC-12.5). If a requirement looks wrong, propose an amendment: a new edition with the reason, the guarantees affected, the alternatives and a migration plan. Founding documents carry versions and exist in English and Russian, and both change together.
- **Interface changes are explicit.** An interface change gets a new version and an explicit transition (MC-12.4, 12.7). In this repository, the system-call ABI lives in `common/abi.rs` and changes only in kernel-track issues. Service interfaces live in `idl/*.wit`.

## 2. Tracks, task numbers and branches

Each track has a **code**, a **range of main-task numbers**, an owner, a set of directories it mainly changes and its branches. A task's number carries its track's code, so tracks never need to coordinate on numbering.

**Task numbers.**

- A **main task** is `NNN`: three digits from its track's range (`issues/NNN-short-name.md`). It states a goal at the size of a roadmap step.
- A **task** of a track is `NNN-TRK-MMMM` (`issues/NNN-TRK-MMMM-short-name.md`):
  - `NNN` is the main task it belongs to. It may be another track's main task, for example `158-DRV-0003` is the drivers track's part of the video task 158. It is `000` for a task under no main task.
  - `TRK` is the track's code.
  - `MMMM` is the track's own counter: four digits, never reused within the track.
- Two tracks never produce the same number, because the codes differ. A main task is split into tasks by any track that works on it: each numbers its own part.
- Numbers given before this scheme (`158`, `205`, `u015`, …) stay as they are.

The tracks — codes, ranges, directories, owners, branches and starting tasks — are listed in the registry [TRACKS.md](TRACKS.md). Today they are `KRN` (kernel), `PRT` (porting), `NET` (network), `APP` (tools), `DRV` (drivers), `STO` (state and recovery), `UPD` (update and provenance), `MRN` (Marain), `SAF` (safety plane), `ASR` (assurance) and `APL` (Apple Silicon).

The registry of tracks — current owners, branches, statuses and starting tasks — is [TRACKS.md](TRACKS.md), the authoritative record. Open tracks can be taken in parallel.

**To join:**

1. **Take an open track** (status "open" in [TRACKS.md](TRACKS.md)): tell the maintainer, who records you as its owner there. Start from the track's main task, if it has one, or propose one.
2. **Or join an owned track** by agreeing with its owner. You use the track's code and counter, and work on your own branch, never on the owner's.
3. **Or ask for a new track.** Open a GitHub issue titled `Track proposal: <name>` that names:
   - the roadmap track or Constitution articles it serves;
   - the directories it will change;
   - what it needs from other tracks.

   The maintainer gives it a code and a range (from 600, in blocks of 50) and records it in [TRACKS.md](TRACKS.md). Until then, propose work as GitHub issues and do not create files in `issues/`.

**Branches.** The branch name carries the tool and then the track code or the task number: `<tool>/<TRK>-<name>` for a track's long-lived branch (`claude/NET-stack`), `<tool>/<NNN-TRK-MMMM>-<name>` for one task (`codex/300-STO-0002-cid`). CI runs on pushes to `main`, to `claude/**` and to branches named this way under any tool prefix, and on pull requests. An agent:

- pushes only to its own branch;
- never force-pushes a branch another session uses;
- never rewrites history that has reached `main`.

## 3. The cycle of one task

1. **Pick or write an issue.** One file per task: `issues/NNN-TRK-MMMM-short-name.md`, or `issues/NNN-short-name.md` for a main task (section 2). It contains:
   - a title;
   - a metadata line: Type · Owner · Priority · Status · Blocked by · Roadmap · Constitution;
   - the sections Problem, Plan, Acceptance criteria, Related.

   Add the issue to the table in `issues/README.md` in the same commit.
2. **Bring in `main` first** (`git fetch origin && git merge origin/main`), so that you build on the other tracks' latest work.
3. **Implement it, following the rules in CONTRIBUTING.md:**
   - comments in English, one line, the essence only;
   - code that exists only for superseded hardware is marked `LEGACY:` and listed in [docs/legacy.md](docs/legacy.md);
   - generated IDL files are committed;
   - third-party code is recorded in [THIRD_PARTY.md](THIRD_PARTY.md).
4. **Test.** Run the suites the change touches. A kernel change runs all QEMU suites on 4 CPUs, and the SMP, isolation, heap and services suites on 1 CPU. A change to aarch64 also runs the aarch64 groups. `scripts/ci_local.sh` runs every CI group on your machine.
5. **Update the evidence.** If the change alters a statement in `docs/profile` (a guarantee, the TCB or evidence), update that statement in the same commit. Update the README and `docs/api` when behaviour or interfaces change.
6. **Commit.** One task per commit where possible. The message says what changed and why, and cites the issue. An agent's commits carry a trailer naming the tool and, if there is one, a link to the session (for example `Co-Authored-By:` and a session URL). The person directing the agent is the author of record and accepts the [licence of contributions](CONTRIBUTING.md#licence-of-contributions).
7. **Close the issue** when its acceptance criteria are met:
   - `git mv issues/NNN-TRK-MMMM-x.md issues-done/NNN-TRK-MMMM-x.done` (a main task: `NNN-x`), when the main task's own criteria are met and its tasks are done or split off;
   - append ` — done` to the title and set `Status: done (YYYY-MM-DD)`;
   - add a `## Resolution` section saying what was done and where;
   - fix the relative links and update both tables in `issues/README.md`.

   If only part is done, the issue stays open with a progress section. If the remainder is large, split it into a new issue.
8. **Push your branch, then get the work into `main`** (section 4).

## 4. Getting to `main`

**Agents of the maintainer** push a fast-forward to `main` after a green gate:

```bash
git fetch origin && git merge-base --is-ancestor origin/main HEAD && git push origin HEAD:main
```

If `main` has moved, merge it into your branch first, run the gate again and retry.

The gate is one of the following:

- **GitHub CI is green** on the pushed commit.
- **Or a full local run** when GitHub's hosted runners do not take the jobs. Run `scripts/ci_local.sh --ref <your-branch>`, which tests the branch merged with the current `main` in a temporary worktree. Every group must pass, and the commit or report must say that the gate was local.

**Everyone else** opens a pull request from a fork or branch. The pull request:

- states its issue and the Constitution clauses it touches;
- shows the result of `scripts/ci_local.sh` if CI could not run.

The maintainer or the owning track reviews and merges it.

**`fast-test`: raw commits for tests on hardware.** This branch is `main` plus commits that have not passed the gate yet. Only the maintainer builds it, to try a fix on a real machine at once, without waiting for the tests.

- **An agent never builds, tests or gates `fast-test`** (a strict rule): no build, no test suite, no `scripts/ci_local.sh` on it or on a worktree of it. Builds, tests and gates run only on the agent's own branch.
- **The order:** an agent of the maintainer commits on its own branch and makes sure it builds there; then merges its branch into `fast-test` (or fast-forwards it), resolves any conflict in the merge, and pushes the merge as it is; only then runs the tests and the gate, on its own branch. Never force-push `fast-test`.
- When `main` moves, it is merged into `fast-test`.
- Nothing goes from `fast-test` to `main`. The same commits reach `main` from the agent's own branch, through the gate.
- A build from `fast-test` is not evidence of anything (section 1) until its commits pass the gate.

**A maintainer's machine can run the gate continuously.** `scripts/ci_watch.sh` fetches `origin` every 10 minutes. It tests each new commit of `main`, and of every other branch merged with `main`, and keeps a history in `~/.cache/mind-ci-watch/history.log`. Run it with `--once` for a single pass.

## 5. Working next to other tracks

- **Stay in your track's directories.** If your task needs a change in another track's area, what the agent does depends on whether that track has an owner in [TRACKS.md](TRACKS.md).
  - **A track with an owner.** The agent:
    1. records a request in `issues/requests-<TRK>.md` for that track (the code in section 2): only the owner numbers its tasks, so it turns each request into an `NNN-TRK-MMMM` task and removes the file when it is empty;
    2. marks its own issue `Blocked by` that request, and then by the task number it gets;
    3. moves on to other work.
  - **An open track** (status "open", no owner). The agent may make the change itself, as a task of that track, when its own task needs it:
    1. After bringing in `main`, it numbers the task with that track's code and the next counter not yet used in `issues/` or `issues-done/` (`NNN-TRK-MMMM`). It writes the task's issue file and names its own task as the reason.
    2. It changes only what its own task needs, in that track's directories, under the same rules and gate as its own work. It does not take the track's main task or its other tasks along.
    3. Its commit names that task and says the work was done for the other track; its own issue links to it.
    4. A request waiting in `issues/requests-<TRK>.md` that the change covers is taken from that file.

    The track stays open. Whoever takes it later owns these tasks as well and may revise them. ABI changes are still made only in `KRN` tasks, and a track marked "later" is not worked on this way.
- **ABI changes are made only in `KRN` tasks.** A task blocked by a `KRN` task waits for it.
- **Resolving conflicts.** When `main` brings a conflict, merge (do not rebase shared history) and keep both sides' behaviour. Regenerate generated files with their tools (`scripts/mind_idl.py`), never by hand. If both sides changed the same logic and choosing one loses behaviour, ask the owner of the other change.
- **Reports from people.** A user report that belongs to another track is passed to that track as described in the first point. It is not fixed silently in passing.
- **Tasks only a person can do** go to [issues-human/](issues-human/README.md): repository settings, legal questions, coordination between agent sessions.

## 6. What an agent must not do

- Push to `main` without a green gate, or push to another track's branch.
- Build, test or run a gate on `fast-test`: it only receives merges of the agents' branches (section 4).
- Skip, disable or weaken a test to get a green result, or push an empty commit to re-trigger CI.
- Change the system-call ABI outside a kernel issue, or change a service interface without a new IDL version.
- Claim a guarantee, profile entry or acceptance criterion that it has not tested on the stated configuration.
- Edit the Constitution, the RFCs or the roadmap in one language only, or without a new version.
- Add third-party code or data without its source and licence, or commit secrets, keys or credentials.
- Post vulnerability details in public. Report them as described in [SECURITY.md](SECURITY.md).

## 7. A brief to give your agent

Paste this at the start of a session and fill in the brackets:

```text
You work on MIND Core (github.com/NikolayUvarov/gcu_undocumented_func) in the <name> track, code <TRK>.
Read AGENTS.md, CONTRIBUTING.md, issues/README.md and the issue you are given before changing anything.
Your branch: <tool>/<TRK>-<name>. Your tasks: NNN-<TRK>-MMMM (your counter starts at <MMMM>); main tasks from <range>.
Your directories: <list>. Requests to a track with an owner go to issues/requests-<THEIR TRK>.md; a change your task needs in an open track (no owner) you may make yourself as that track's task (AGENTS.md section 5).
Never push to other branches but fast-test; reach main only through the gate in AGENTS.md section 4.
Build, test and gate only your own branch: once a commit builds, merge it into fast-test and push, then run the tests on your branch. Never build or test fast-test.
Every change serves a cited Constitution clause (MC-x.y) or roadmap item; do not present plans as guarantees.
Comments in English, one line. One task per commit, citing the issue; close finished issues as issues/README.md says.
Before pushing: run the tests the change touches (scripts/ci_local.sh for the full set) and update docs/profile if a stated guarantee changed.
Work that belongs to another track goes to that track as an issue or report, not into your commit.
```

Reply to people in whatever language they write in. Commits, code, comments and the files in `issues/` are in English, and founding documents are kept in both English and Russian.
