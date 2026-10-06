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

## 2. Tracks, ranges and branches

Each track has an owner, a set of directories it mainly changes, an issue-number range and a branch. Numbers never collide, so tracks never need to coordinate on numbering.

| Track | Roadmap | Issues | Owns mainly |
|---|---|---|---|
| tools | G | `u001`, `u002`, … (a counter of its own) | user tools, `wm`, `fm`, `edit`, `view`, voice |
| network | D | 100–149 | `virtio_net`, `netstack`, `netpolicy`, `tls`, `keystore` |
| kernel | A and the kernel side of the others | 150–199 | `kernel/`, `common/abi.rs`, the core services |
| porting | H | 200–249 | `kernel/src/arch/`, other architectures (with the kernel track) |

The current owners, branches and ranges are listed in [issues/README.md](issues/README.md), which is the authoritative record. Roadmap tracks B (state and recovery), C (update and provenance), E (Marain), F (safety plane) and Assurance have no owner yet: they are the natural places for a new track.

**To join:**

1. **Join an existing track** by agreeing with its owner, through a GitHub issue or the maintainer. You then use that track's range and work on your own branch, never on the owner's.
2. **Or ask for a new track.** Open a GitHub issue titled `Track proposal: <name>` that names:
   - the roadmap track (A–H) or Constitution articles it serves;
   - the directories it will change;
   - what it needs from other tracks.

   The maintainer gives it a number range (250 and up, in blocks of 50) or a prefix, and records it in `issues/README.md`. Until then, propose work as GitHub issues and do not create files in `issues/`.

**Branches.** One branch per agent session or track. The name should show the tool and the track, for example `claude/<name>` or `codex/network-<name>`. CI runs on pushes to `main` and `claude/**` and on pull requests; with another prefix, open a pull request or add the prefix to `.github/workflows/ci.yml`. An agent:

- pushes only to its own branch;
- never force-pushes a branch another session uses;
- never rewrites history that has reached `main`.

## 3. The cycle of one task

1. **Pick or write an issue.** One file per task: `issues/NNN-short-name.md` (the tools track: `uNNN-short-name.md`). It contains:
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
   - `git mv issues/NNN-x.md issues-done/NNN-x.done`;
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

**A maintainer's machine can run the gate continuously.** `scripts/ci_watch.sh` fetches `origin` every 10 minutes. It tests each new commit of `main`, and of every other branch merged with `main`, and keeps a history in `~/.cache/mind-ci-watch/history.log`. Run it with `--once` for a single pass.

## 5. Working next to other tracks

- **Stay in your track's directories.** If your task needs a change in another track's area, the agent:
  1. writes an issue for that track in its range, or records a report in `issues/` the way [tools-track-reports.md](issues/tools-track-reports.md) does when the owner numbers its own issues;
  2. marks its own issue `Blocked by` that issue;
  3. moves on to other work.
- **ABI changes are made only in kernel-track issues.** A tools issue blocked by a kernel issue waits for it.
- **Resolving conflicts.** When `main` brings a conflict, merge (do not rebase shared history) and keep both sides' behaviour. Regenerate generated files with their tools (`scripts/mind_idl.py`), never by hand. If both sides changed the same logic and choosing one loses behaviour, ask the owner of the other change.
- **Reports from people.** A user report that belongs to another track is passed to that track as described in the first point. It is not fixed silently in passing.
- **Tasks only a person can do** go to [issues-human/](issues-human/README.md): repository settings, legal questions, coordination between agent sessions.

## 6. What an agent must not do

- Push to `main` without a green gate, or push to another track's branch.
- Skip, disable or weaken a test to get a green result, or push an empty commit to re-trigger CI.
- Change the system-call ABI outside a kernel issue, or change a service interface without a new IDL version.
- Claim a guarantee, profile entry or acceptance criterion that it has not tested on the stated configuration.
- Edit the Constitution, the RFCs or the roadmap in one language only, or without a new version.
- Add third-party code or data without its source and licence, or commit secrets, keys or credentials.
- Post vulnerability details in public. Report them as described in [SECURITY.md](SECURITY.md).

## 7. A brief to give your agent

Paste this at the start of a session and fill in the brackets:

```text
You work on MIND Core (github.com/NikolayUvarov/gcu_undocumented_func) in the <name> track.
Read AGENTS.md, CONTRIBUTING.md, issues/README.md and the issue you are given before changing anything.
Your branch: <branch>. Your issue numbers: <range or prefix>. Your directories: <list>.
Never push to other branches; reach main only through the gate in AGENTS.md section 4.
Every change serves a cited Constitution clause (MC-x.y) or roadmap item; do not present plans as guarantees.
Comments in English, one line. One task per commit, citing the issue; close finished issues as issues/README.md says.
Before pushing: run the tests the change touches (scripts/ci_local.sh for the full set) and update docs/profile if a stated guarantee changed.
Work that belongs to another track goes to that track as an issue or report, not into your commit.
```

Reply to people in whatever language they write in. Commits, code, comments and the files in `issues/` are in English, and founding documents are kept in both English and Russian.
