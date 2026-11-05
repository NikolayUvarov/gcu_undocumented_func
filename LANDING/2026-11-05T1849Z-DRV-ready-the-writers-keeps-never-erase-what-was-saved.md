# Ready: 800-LNK-0036, a damaged old disk never erases what `build_and_write.sh` kept

**From:** LNK (DRV's session) · **To:** LANDING · **Written:** 2026-11-05 18:49 UTC · **Concerns:** 800-LNK-0036 (closes)

**Ready:** `claude/ASR-DRV` 61167c2f

It contains `main` 0c2e32a3. DRV's last request, 1432Z, landed in 6253e7a7; this one adds 61167c2f.

## What it brings

`scripts/build_and_write.sh`'s `--keep-effector`, `--keep-settings` and `--keep-wifi` no longer trust the old disk completely:
- a part or file of it that is missing or empty keeps what was saved before, and the script says so;
- a replaced copy stays as `<dir>.prev`, or `<file>.prev` for the keystore's files.

On 2026-11-05 a damaged Ryzen disk without `data/effector` made the save step wipe the owner's agent configuration on LEG9.

`tests/build_and_write_keep_test.sh` takes the functions from the script by name and runs them on plain directories, 16 checks. `scripts/host_tests.sh` runs it.

## Tested, on LEG9

- All of `scripts/host_tests.sh` (with `MIND_SSHD`) on 61167c2f: exit 0, 188 suites, the keep test among them.
- `bash -n` on the script, and `tests/build_and_write_args_test.py`.
- The branch already contains `main` 0c2e32a3, so there was nothing to merge.
- No disk was written with this version yet. The maintainer's next `fast-test` write is its first run; it is in `fast-test` (c9d8b061).
