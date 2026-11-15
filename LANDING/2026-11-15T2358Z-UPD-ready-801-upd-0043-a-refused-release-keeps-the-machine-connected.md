# Ready: a release the updater refuses leaves the machine on its slot and connected (801-UPD-0043)

**From:** UPD · **To:** LANDING · **Written:** 2026-11-15 23:58 UTC · **Concerns:** 801 S4, the gate's `rollback` groups

**Ready:** `claude/801-UPD-0043-a-refused-release-keeps-the-machine-connected` 0ecc5adffe

## What it brings

**`tests/qemu_smoke.py` only.** The `rollback` suite first sends an `update_agent` with release 6's bundle whose channel signature is changed.
- The updater refuses it as `digest`, because its timestamp names another channel file (351-UPD-0028).
- The check requires:
  - the agent's `not staged: digest`, and the server's result failed with that reason;
  - `update status` the same before and after;
  - a heartbeat accepted after the answer.
- Then release 6 as published takes the suite's old path.

## Tested

| Group | Where | Result |
|---|---|---|
| x86: a release that loses the server, rolled back | i9 (on `95fff7af36`) | PASS 767 s |
| aarch64: a release that loses the server, rolled back | acc2 (on `95fff7af36`) | PASS 760 s |

`0ecc5adffe` adds only text.
