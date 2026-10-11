# 000-KRN-0088 — `mind-core-os`: a copy of this repository under the OS's own name, kept up to date from `main`

**Type:** repository and CI (`scripts/mirror_showcase.sh`, `.github/workflows/mirror.yml`, README) · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (both repositories made, `main` copied once by hand; the token for the workflow waits for the maintainer) · **Blocked by:** the maintainer's steps below · **Roadmap:** — · **Constitution:** —

## Problem

The maintainer (2026-10-11) wants the project found by as many words as possible. The repository's name, `gcu_undocumented_func`, says nothing about it, but it is to stay: the agents, Effector's clones and the sessions use it.

The decision:

- `mind-core-os`: a copy of this repository under the OS's name, kept up to date.
- `mind-core`: a new repository for the Mind itself, the intelligence module.

## Plan

- **Work stays here** (the maintainer: this repository is the main one). `mind-core-os`, the showcase, gets `main` and the tags only, and is never written to by hand.
- **The script `scripts/mirror_showcase.sh`** copies them:
  - only when `main` or the tags differ from the showcase's;
  - with `--every-hours N`, only when the showcase's last push (GitHub's `pushed_at`) is at least N hours old;
  - with a plain push, never forced, so a commit made in the showcase stops the copy rather than being lost.

  Anyone with write access to the showcase may run it by hand.
- **The maintainer's scripts** (the maintainer, 2026-10-11: "make a script for me, I will run the sync"), run from any clone with their own git access:
  - `06_sync_showcase.sh` (Linux, macOS, WSL, Git Bash), which calls `scripts/mirror_showcase.sh` with the showcase's SSH address;
  - `06_sync_showcase_windows.ps1` (PowerShell), the same steps.
- **The workflow `.github/workflows/mirror.yml`:**
  - looks once a day and runs the script with `--every-hours 48`: the showcase changes at most every 48 hours, and only when something changed (the maintainer, 2026-10-11);
  - run by hand (Actions → Run workflow), it copies any change at once;
  - it pushes with the secret `MIRROR_TOKEN` (a fine-grained token that may write only to `mind-core-os`) or `MIRROR_SSH_KEY` (a deploy key of `mind-core-os`); without either it copies nothing and does not fail. The maintainer's own SSH key is not used: GitHub refuses a key of an account as a deploy key, and in a secret it would let any workflow change write to every repository of the account;
  - in the showcase itself it does not run.
- **Agents' branches and `fast-test`** are not copied. What reaches `main` has passed the gate, so the copy holds only gated work.
- **The maintainer's steps** (the GitHub app may not create repositories: "403 Resource not accessible by integration"):
  1. create `mind-core-os`, public and empty, and `mind-core`, public;
  2. give the Claude GitHub app access to both;
  3. a fine-grained token for `mind-core-os` alone (Contents: read and write), kept as this repository's Actions secret `MIRROR_TOKEN`; or else a deploy key, the secret `MIRROR_SSH_KEY`.
- **Then the kernel session:**
  - pushes `main` and the tags to `mind-core-os` once;
  - names both repositories in README.md;
  - writes `mind-core`'s first README.

## Progress

**2026-10-11.**

- The maintainer made both repositories and gave the GitHub app access.
- `main` (f7e7a8a, no tags yet) was pushed to `mind-core-os` once by hand.
- `mind-core` got its first README.
- Both of the maintainer's scripts were run against a local bare repository: the first copy, then "nothing changed"; the PowerShell one (7.4) refused to overwrite a commit made in the showcase.
- `tests/mirror_test.py` checks the script on local repositories:
  - the first copy;
  - nothing done without a change;
  - a change that waits under 48 hours and goes after;
  - a commit in the showcase that stops the copy.

## Acceptance criteria

- `mind-core-os` holds this repository's `main` and tags.
- A change to `main` here reaches `mind-core-os` through the workflow, at most 48 hours after the last copy.

## Related

[AGENTS.md](../AGENTS.md) §4 (getting to `main`).
