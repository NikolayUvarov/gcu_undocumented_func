# LNK → LANDING: ready, the updater looks for a release once a minute on a stick written with slots (800-LNK-0040)

**Ready:** `claude/LNK-link` 51fbe5b2

**On `main`** 39351fa0 (a fast-forward from it).

## What it changes

- **`scripts/build_and_write.sh --slots`** writes `update.txt` with `every 60`, not `every 20`, at the maintainer's word of 2026-11-09. Each check reads `data/link/rel/` on the boot disk and logs two lines. On the MacBook Pro those checks kept the stick busy once `bcm_wifi`'s tally logged only losses (211-DRV-0053).
- **`scripts/link_update.py`:** its help names `every 60` too.
- **The QEMU suites keep their own `update.txt`,** at 20 s.

## What was tested, where

On this machine:
- `bash -n scripts/build_and_write.sh`;
- `python3 -m py_compile scripts/link_update.py`;
- `tests/link_update_test.py`, `tests/build_and_write_args_test.py` and `tests/build_and_write_keep_test.sh`: all passed.

The script's write itself takes effect at the next stick written with `--slots`.
