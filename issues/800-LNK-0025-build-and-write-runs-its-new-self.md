# 800-LNK-0025 — `build_and_write.sh` runs its new self after the pull, with the same arguments

**Type:** tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-10-24) · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-12.1

## Problem

The maintainer's word (2026-10-24, through EFF's `LNK/2026-10-24T1120Z-EFF-…build-and-write-runs-its-new-self-after-the-pull.md`, 97316893): "if the script updates itself from the repository while it runs, it must restart with the same arguments, or the previous version may run."

- **The copy comes first.** The script copies itself at its start and runs from the copy, since bash reads a script as it runs. But the copy is of the version present at that moment.
- **The update comes after it.** Then `git checkout fast-test` and `git pull --ff-only` bring the new version into the clone. The run goes on in the old copy, with the old options and the old logic.

## What was done

1. **The arguments as given** are kept at the start (`ORIG_ARGS`).
2. **After the checkout or the pull,** when the clone's script (`$SELF`) differs from the running copy, the script says so: `>>> build_and_write.sh changed with the pull: running the new one`.
   - It removes its copy, since `exec` runs no EXIT trap, and runs the new script with the same arguments.
   - That happens before anything is built, saved or written. The old disk's save is therefore always made by the version that writes the disk.
3. **The new run** (`BUILD_AND_WRITE_UPDATED=1`) does not fetch, check out or pull again, so it does not loop.
4. **With `--commit`,** the commit's own script runs. The new run gives the branch back at its exit, through `BUILD_AND_WRITE_RESTORE_BRANCH`. A commit older than this change gives it back itself, as it always has.
5. **Every run says which script runs:** `>>> build_and_write.sh <commit> runs`, with `, the version the pull brought` or `, the commit's own` after a re-run.

**Checked** in a sandbox repository, on the script's own blocks (the copy, the arguments, the update), with a stand-in for the build:
- the clone at v1, `origin` at v2: one re-run; v2 ran with the 3 words given, `"a b"` kept whole; no copy left in `/tmp`;
- nothing new: no re-run;
- `--commit` of v1 from `main` at v2: v1's script ran, and the clone was back on `main` at its exit.

## Acceptance criteria

- [x] The sandbox checks above.
- [ ] The maintainer's next write after a change to the script shows `running the new one`, and the new version's options work in it.

## Related

[800-LNK-0023](800-LNK-0023-keep-the-effector-agent-across-a-write.md), [800-LNK-0024](800-LNK-0024-keep-the-owners-settings-across-a-write.md), 800-LNK-0014.
