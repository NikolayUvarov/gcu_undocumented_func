# LNK landing request: `link_update.py` takes no cut read of the log for the next boot's (000-LNK-0049)

**From:** LNK · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-20 21:14 UTC · **Concerns:** 000-LNK-0049

**Ready:** `claude/000-LNK-0049-a-cut-read-is-not-a-new-boot` 419a9f5f2

## What it carries

On `main` 03c88c514; in `fast-test` 4eb1e7af2.

- **`scripts/link_update.py`.** On the DEXP it said `CONFIRMED` and left with 0 while the release it had put was only staged. The line it gave was the confirmation of the update before.
  - **Why:** a read of the log came back cut short as the machine restarted. It began at the boot's start and ended before the last line read, so it was taken for the next boot's log.
  - **Now:** a read that is a part of the log as last read whole gives no new line. A confirmation counts only for the slot the release was staged in and for a boot that tries this version.
- **`tests/link_update_test.py`:** the DEXP's sequence as a host test. It fails with the script as it was.
- **Text:** the issue, written as done, and its row.
- **A tool only:** nothing of it goes into an image. No ABI or IDL change.

## Tested

- **On LEG9, this commit:** `build (x86)` and `host tests` pass, `tests/link_update_test.py` among them.
- **On the DEXP:** the next release through the cable (202611202012) was followed to the confirmation of the slot it was staged in.
- **Not tested on a machine:** the cut read itself. It came once, at a restart, and cannot be asked for.
