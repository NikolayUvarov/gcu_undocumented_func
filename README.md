# Messages between tracks

This branch holds questions, answers and notices between MIND Core's tracks ([TRACKS.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/TRACKS.md) on `main`) and the maintainer. It was opened at the maintainer's decision on 2026-10-14.

- It has no code and shares no history with `main`.
- Nothing here is built, tested or merged into another branch.
- The rules are in [AGENTS.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/AGENTS.md), section 5. This file says how to use the branch.

## What goes here, and what does not

- **Here:**
  - a question to another track or to the maintainer, and its answer;
  - a notice ("550-KRN-0097 is in `fast-test`", "the request is recorded in `requests-NET.md` at 4d997db");
  - a remark on another track's text or code;
  - a claim of an open track or a track proposal (to `HUMAN`), or an agreement to join an owned track (to its owner). The outcome is recorded in `TRACKS.md` on `main` (AGENTS.md, section 2).
- **On `main`, not here:** anything binding, and the message points to it.
  - A request for work goes in `issues/requests-<TRK>.md`.
  - A decision goes in the issue it concerns.
  - A task only a person can do goes in `issues-human/`.
- **Nowhere in this repository.** This branch is as public as the repository, so none of these goes here:
  - secrets, keys or credentials, the Effector API key among them;
  - proprietary files;
  - vulnerability details ([SECURITY.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/SECURITY.md)).

## Layout

One file per message, never edited after it is pushed:

```text
<TO>/<YYYY-MM-DDTHHMMZ>-<FROM>-<subject>.md
```

- **`<TO>`** is where the message goes:
  - the recipient's track code (`KRN`, `NET`, `DRV`, …);
  - `ALL`, for every track or for several, which the header then names;
  - `HUMAN`, for the maintainer.
- **The time** is UTC, when the message is written, so a directory's files sort in order.
- **`<FROM>`** is the writer's track code, or `HUMAN`.
- **`<subject>`** is a few words, lower case, joined by hyphens.

A message:

```markdown
# The stack waits for the station

**From:** KRN · **To:** NET · **Written:** 2026-10-14 12:30 UTC · **Answers:** — · **Concerns:** 550-NET-0018, `main` 4d997db · **Answer wanted:** yes

What is asked or told, with the facts and where each comes from.
```

- **An answer** is a new file in the asker's directory. Its **Answers** field names the question's path. A question stays open until a file answers it.
- **Language.** Messages are in English, as `issues/` are. The maintainer writes in any language.
- **Facts.** A fact names its source: a commit, a log, a file and line. A guess is marked as one (MC-12.3).

## Reading

At the start of every session and before each task, read your track's directory and `ALL`:

```bash
git fetch origin internal-dev-messages
# every message to you
git ls-tree -r --name-only origin/internal-dev-messages -- KRN ALL
# what is new since the last commit you read
git log --reverse --name-only --format='%h %ci %s' <last read>..origin/internal-dev-messages -- KRN ALL
```

Keep the last commit you read where your session keeps its notes.

## Writing

Write in a worktree of this branch, never in your track's branch:

```bash
git fetch origin internal-dev-messages
git worktree add --detach <dir> origin/internal-dev-messages
# write <dir>/NET/2026-10-14T1230Z-KRN-the-stack-waits.md, then:
git -C <dir> add NET/2026-10-14T1230Z-KRN-the-stack-waits.md
git -C <dir> commit    # "KRN to NET: the stack waits for the station", with AGENTS.md's trailers (section 3)
git -C <dir> push origin HEAD:internal-dev-messages
# refused because the branch moved:
git -C <dir> pull --rebase origin internal-dev-messages && git -C <dir> push origin HEAD:internal-dev-messages
```

- **One message per commit.**
- **Rebasing your own unpushed message is the only rewriting allowed.** Never force-push, never merge this branch into another or another into it, and never edit or delete a pushed message.
- **A wrong message is corrected by a new one.**

## The landing queue: `LANDING/`

Code reaches `main` through the landing queue (AGENTS.md, section 4, the maintainer's decision of 2026-10-21). Its requests and answers are messages in `LANDING/`, written and read like the others.

- **A request** from a track whose branch is ready, tested and has `main` merged in:

  ```markdown
  # Ready: the link's developer mode

  **From:** LNK · **To:** LANDING · **Written:** 2026-10-21 10:00 UTC · **Concerns:** 800-LNK-0012

  **Ready:** `claude/LNK-link` 1234567890abcdef1234567890abcdef12345678

  What it closes or advances, and what was run, where.
  ```

  The file is `LANDING/<YYYY-MM-DDTHHMMZ>-<TRK>-ready-<subject>.md`. A later request for the same branch replaces one not yet answered.
- **The answer** comes from the integrator, `scripts/land.sh announce`: one file per landing round, `LANDING/<time>-KRN-landed-<candidate>.md` or `…-round-…`, with a line for each request it settles:
  - `**Landed:**` — in `main` now;
  - `**Returned:**` — with the reason: a conflict, or the failure it caused;
  - `**Superseded:**` — a newer request of the branch replaced it.

  A request with no answer waits for the next round.
- **Read** `LANDING/` with your directory and `ALL`: `git ls-tree -r --name-only origin/internal-dev-messages -- LANDING`.

## Why a branch

- **`main` stays still.** Messages on `main` would move it, and every move sends the tracks with code back to merge `main` and run their gate again (AGENTS.md, section 4).
- **No CI.** Its name matches none of CI's patterns, so CI does not run on it.
- **Not mirrored.** The mirror to mind-core-os copies only `main`.
