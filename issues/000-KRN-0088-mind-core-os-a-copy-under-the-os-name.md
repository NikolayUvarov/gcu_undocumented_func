# 000-KRN-0088 — `mind-core-os`: a copy of this repository under the OS's own name, kept up to date from `main`

**Type:** repository and CI (`.github/workflows/mirror.yml`, README) · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (the workflow made; waits for the maintainer: the repositories, the GitHub app's access, the deploy key) · **Blocked by:** the maintainer's steps below · **Roadmap:** — · **Constitution:** —

## Problem

The maintainer (2026-10-11) wants the project found by as many words as possible. The repository's name, `gcu_undocumented_func`, says nothing about it, but it is to stay: the agents, Effector's clones and the sessions use it.

The decision:

- `mind-core-os`: a copy of this repository under the OS's name, kept up to date.
- `mind-core`: a new repository for the Mind itself, the intelligence module.

## Plan

- **Work stays here.** `mind-core-os` gets `main` and the tags only, and is never written to by hand.
- **The workflow `.github/workflows/mirror.yml`:**
  - on every push to `main` or of a tag, it pushes them to `mind-core-os` with a deploy key (the secret `MIRROR_SSH_KEY`);
  - the push is plain, never forced, so a commit made in the copy stops the copy rather than being lost;
  - without the secret it pushes nothing and does not fail;
  - in the copy itself it does not run.
- **Agents' branches and `fast-test`** are not copied. What reaches `main` has passed the gate, so the copy holds only gated work.
- **The maintainer's steps** (the GitHub app may not create repositories: "403 Resource not accessible by integration"):
  1. create `mind-core-os`, public and empty, and `mind-core`, public;
  2. give the Claude GitHub app access to both;
  3. make an ed25519 key pair. The public key goes to `mind-core-os` as a deploy key with write access, the private key to this repository's Actions secret `MIRROR_SSH_KEY`.
- **Then the kernel session:**
  - pushes `main` and the tags to `mind-core-os` once;
  - names both repositories in README.md;
  - writes `mind-core`'s first README.

## Acceptance criteria

- `mind-core-os` holds this repository's `main` and tags.
- A push to `main` here reaches `mind-core-os` through the workflow.

## Related

[AGENTS.md](../AGENTS.md) §4 (getting to `main`).
