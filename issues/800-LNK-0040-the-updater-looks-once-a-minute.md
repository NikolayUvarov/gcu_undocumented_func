# 800-LNK-0040 — A stick written with slots has its updater look for a release once a minute, not every 20 s

**Type:** script · **Owner:** `LNK` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-6.1

## Problem

`scripts/build_and_write.sh --slots` writes `update.txt` with `every 20`. Each of the updater's checks:
- reads its source, `data/link/rel/`, on the boot disk;
- logs two lines, and so brings a save of the system log.

On the MacBook Pro on 2026-11-09 these checks were what kept the stick busy, once `bcm_wifi`'s tally logged only losses ([211-DRV-0053](211-DRV-0053-a-usb-disk-that-missed-a-transfer-comes-back.md)).

**The maintainer** (2026-11-09): «обновления давай проверять реже, раз в минуту» (let the updates be checked less often, once a minute).

## Plan

- **`build_and_write.sh`:** `every 60`.
- **`link_update.py`:** the `update.txt` its help names says `every 60` too. A release put over the link is then taken up within a minute; the script follows the log for up to 1800 s, so nothing else changes.
- **The QEMU suites keep their own `update.txt`,** at 20 s, for speed.

**Where it applies.** `update.txt` lies at the boot volume's root, which nothing inside MIND Core writes, so the period changes only with the stick's next write.

## Acceptance criteria

- The next stick written with `--slots` holds `every 60`.
- On the machine booted from it, the updater's checks come a minute apart in the log.

## Related

[211-DRV-0053](211-DRV-0053-a-usb-disk-that-missed-a-transfer-comes-back.md), [docs/update/updater.md](../docs/update/updater.md), [scripts/link_update.py](../scripts/link_update.py).
