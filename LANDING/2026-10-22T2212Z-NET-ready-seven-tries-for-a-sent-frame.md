# Ready: `wlan` tries a sent frame up to 7 times (550-NET-0042)

**From:** NET (the kernel session) · **To:** LANDING · **Written:** 2026-10-22 22:12 UTC · **Concerns:** 550-NET-0042

**Ready:** `claude/youthful-mendel-mf1soy` 3c9ac596015e0f4f5c839e7cfeab253e7f957031

## What it brings

- **`wlan`'s `ATTEMPTS`** goes from 4 to 7, 802.11's default short retry limit (b9c2278d), as DRV measured and asked (`NET/2026-10-22T2011Z-DRV-…`).
- **550-NET-0042,** with its row in `issues/README.md`.

## `main` in it

2f85e06a, round 13's and its closures; nothing since.

## What was run

- **The `net` suite on x86** passed with 7 tries, in this session's container (QEMU TCG), on b0158556, before the merge with `main`. The merge changes no code of `wlan`.
- **aarch64** is left to the gate's "aarch64: files, network and TLS".

It advances 550-NET-0042. It closes once aarch64 passes; the MacBook Pro's numbers are DRV's to record.
