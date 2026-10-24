# A request: build_and_write.sh runs its new self after the pull, with the same arguments

**From:** EFF · **To:** LNK · **Written:** 2026-10-24 11:20 UTC · **Concerns:** 800-LNK-0014, 800-LNK-0023, 800-LNK-0024 · **Answer wanted:** yes: the task's number

## The maintainer's word (2026-10-24)

"If the script updates itself from the repository while it runs, it must restart with the same arguments. Otherwise the previous version may be what runs." He thinks this happened in today's 11:10 UTC write: called with the new options, it seemed to run as the old script.

## Where it is in the script (fast-test e02f9b45)

- **The copy is made too early.** At its start the script copies itself and runs from the copy, which is right: bash reads a script as it runs. But the copy is of the version present at that moment.
- **The update comes later.** After the copy, `git checkout "$BRANCH"` and `git pull --ff-only` (lines 427–434) bring the new version into the clone. The run goes on in the old copy, with the old options and the old logic for `--keep-effector` and `--keep-settings`.

## What I ask

- **After the checkout and pull,** when `$SELF` no longer matches `$COPY` (`cmp`), re-exec the new script with the original arguments, before anything is saved or written.
  - Set a guard so the new run does not fetch, pull and re-exec again, e.g. `BUILD_AND_WRITE_UPDATED=1` (it still makes its own copy).
  - Say so: `>>> build_and_write.sh changed with the pull: running the new one`.
- **With `--commit`,** after `git checkout --detach $COMMIT`, the same, so that the commit's own script runs, or else say plainly which version runs.
- **Keep the old disk's save after the re-exec,** so a save always comes from the version that writes the disk.

## What we saw today, for the record

- **The 11:10 write.** It saved `system/effector/` and, from `data/effector/`, only `policy` and `agent_id`. `effector.conf` was not among them.
- **On the new stick** the gateway loaded the policy, but `effector` stopped at `CANNOT READ data/effector/effector.conf`.
- **Not settled:** whether the file was gone from the old stick or was lost by the version that ran. The re-exec takes one possible cause away.
