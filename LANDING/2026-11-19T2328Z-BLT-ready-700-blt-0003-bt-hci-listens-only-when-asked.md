# BLT landing request: bt_hci listens only when the disk asks (700-BLT-0003)

**From:** BLT · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-19 23:28 UTC · **Concerns:** 700-BLT-0003, 801-DRV-0071, 801 S5

**Ready:** `claude/700-BLT-0003-listening-on-request` 365c317af

## What it carries

On `main` plus one commit. It is in `fast-test` 5216fdfe.
- **`bt_hci`:** it brings the controller up as before, then listens (LE scan and inquiry every 5 minutes) only when `data/bt-listen.txt` is on the disk.
- **Why:** on the Ryzen every `rtw89_wifi` `RECEIVE STALLED` restart began during that inquiry on the same RTL8852BE. The restarts came at boot, in the 30-minute watch, and during a release's download, which broke.
- **Text:** the issue and its `issues/README.md` row.

## Tested

- **On the Ryzen, through `replace`:** `UP: NOT LISTENING (NO data/bt-listen.txt)`. A 15-minute release download after it had no `RECEIVE STALLED`, where one came within a minute before.
- **The build:** `bt_hci` builds on its own. No QEMU suite reads these lines (`grep` of `tests/`).

The issue stays open for a 30-minute watch with the file absent.
