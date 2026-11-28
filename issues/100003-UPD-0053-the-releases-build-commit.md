# 100003-UPD-0053 — A release's build commit, published beside it: `releases/<version>/COMMIT`

**Type:** release maker (`scripts/release.py`) · **Owner:** `UPD` (the kernel session) · **Priority:** P2 · **Status:** in progress (made, 2026-11-28) · **Blocked by:** — · **Main task:** [100003](100003-mind-hub-requests-conversation-check-and-updates.md) · **Constitution:** MC-9.2, MC-9.6, MC-12.3

## Problem

- MIND Hub tells a user that the request they made is delivered once a release carries the task that took it ([docs/hub/README.md](../docs/hub/README.md), 8.4).
- To know which tasks a release carries, the hub reads `issues-done/` between the previous release's commit and this one. That needs each release's build commit on the server.
- The hub asked for it as "the release maker publishes it beside the release, in `releases/<version>/COMMIT` (UPD's part, asked)".

## Plan

- **Where it comes from.** Each architecture's boot manifest already names the build's commit in its `commit` line, which `scripts/sign_manifest.py` writes from `git rev-parse HEAD` and the boot key signs.
- **`release.py stage`** writes `releases/<version>/COMMIT`: that commit, 40 hex digits and a newline.
  - `publish` uploads it with the rest of `releases/<version>/`.
  - Architectures whose manifests name two commits are refused before anything is staged.
  - A build that did not know its commit (`commit unknown`) gets no `COMMIT`, says so on stderr, and is released all the same.
- **What it is for.** The file links a release to its tasks; it is not a guarantee of anything (MC-12.3). Its authenticity is the manifest's, which the boot key signs; the file beside it is not signed of its own.

## Acceptance criteria

1. **The tests** in `tests/release_test.py`, `test_a_release_names_the_commit_its_manifests_name`:
   - a published release has `releases/1/COMMIT`, and the server still checks;
   - two architectures from two commits are refused, with nothing staged;
   - a build of unknown commit is released without `COMMIT`.

   **Met on the branch** (19 tests, one skipped as before).
2. MIND Hub reads it: APP's part (100003-APP-0159's tree), told on `internal-dev-messages`.

## Related

[docs/update/publishing.md](../docs/update/publishing.md) (the server's layout), [352-UPD-0050](352-UPD-0050-a-releases-artifacts-from-one-commit.md), 100003-APP-0159, [docs/hub/README.md](../docs/hub/README.md) 8.4.
