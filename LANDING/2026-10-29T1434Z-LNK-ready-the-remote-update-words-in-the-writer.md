# Ready: `build_and_write.sh --mode=` takes `remote-update` and `link-remote-update` (751-LNK-0030, the script's part)

**From:** LNK · **To:** LANDING · **Written:** 2026-10-29 14:34 UTC · **Concerns:** 751-LNK-0030 (advanced), main task 751

**Ready:** `claude/LNK-link` dfbe159891885511561ab1ca375566843c5fe991

## What it brings

- **`scripts/build_and_write.sh --mode=`:**
  - takes `remote-update` beside `development` or `maintenance`;
  - takes `link-remote-update` beside `link-console`, which still needs `--link-dev`;
  - refuses either otherwise.

  The words are `init`'s, from KRN's 751-KRN-0185. Until it lands, `init` logs them as left out, and the docs say so.
- **`tests/build_and_write_args_test.py`** (5 tests, in `scripts/host_tests.sh`):
  - it runs the refusals with a stand-in `wslpath`;
  - each ends the script right after its arguments, before anything is built or written, and leaves no copy behind.
- **`docs/write-disk.md`,** both languages: the words.
- **The issue:** progress recorded in 751-LNK-0030.
- **Interfaces:** no ABI change. No IDL change.
- **`main` in it:** f685cefe.

## What was run on this branch

- `tests/build_and_write_args_test.py`: 5 OK, after the merge with `main`;
- `bash -n scripts/build_and_write.sh`.

Nothing else is touched: the script's other paths are as on `main`.
