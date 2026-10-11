# 000-KRN-0088 — `mind-core-os`: a copy of this repository under the OS's own name, kept up to date from `main`

**Type:** repository and CI (`scripts/mirror_showcase.sh`, `.github/workflows/mirror.yml`, README) · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (both repositories made, `main` copied once by hand; the deploy key waits for the maintainer) · **Blocked by:** the maintainer's steps below · **Roadmap:** — · **Constitution:** —

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
- **The workflow `.github/workflows/mirror.yml`:**
  - looks once a day and runs the script with `--every-hours 48`: the showcase changes at most every 48 hours, and only when something changed (the maintainer, 2026-10-11);
  - run by hand (Actions → Run workflow), it copies any change at once;
  - it pushes with a deploy key, the secret `MIRROR_SSH_KEY`; without the secret it copies nothing and does not fail;
  - in the showcase itself it does not run.
- **Agents' branches and `fast-test`** are not copied. What reaches `main` has passed the gate, so the copy holds only gated work.
- **The maintainer's steps** (the GitHub app may not create repositories: "403 Resource not accessible by integration"):
  1. create `mind-core-os`, public and empty, and `mind-core`, public;
  2. give the Claude GitHub app access to both;
  3. make an ed25519 key pair. The public key goes to `mind-core-os` as a deploy key with write access, the private key to this repository's Actions secret `MIRROR_SSH_KEY`.
- **Then the kernel session:**
  - pushes `main` and the tags to `mind-core-os` once;
  - names both repositories in README.md;
  - writes `mind-core`'s first README.

## Progress

**2026-10-11.**

- The maintainer made both repositories and gave the GitHub app access.
- `main` (f7e7a8a, no tags yet) was pushed to `mind-core-os` once by hand.
- `mind-core` got its first README.
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
