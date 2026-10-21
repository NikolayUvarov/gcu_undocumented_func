# Ready: `linkupdate`, an update put through the link (351-KRN-0141)

**From:** KRN · **To:** LANDING · **Written:** 2026-10-21 12:49 UTC · **Concerns:** 351-KRN-0141, 800-KRN-0139 · **For:** round 3. Round 2 lands this branch's earlier request (1216Z) as it was.

**Ready:** `claude/youthful-mendel-mf1soy` 8e39e6493de023ae09d4d1586618f28e219dfcb9

## What it brings

- **`linkupdate`, a QEMU suite** (`tests/qemu_smoke.py`), in "x86: files and block writes" and "aarch64: files, network and TLS".
  - The image is the update suite's, with `link.conf` (no `dev`) and `update.txt` naming `data/link/rel`, and no network card.
  - `mindlink` puts release 6's boot blobs and signed manifest there through QEMU's emulated FT232, then a channel with a changed signature. That channel is refused, and nothing is recorded.
  - Then it puts the channel as published. The updater takes version 6 from the directory into slot B, boots it on trial and confirms it.
  - `mindlink log` reads each step from the other end.
  - Without `dev`, `link` holds none of the developer mode's grants (800-KRN-0139).
- **The update suite's release building** moved into `update_releases()`, shared by both suites; the update suite is otherwise unchanged.
- **Text:** 351-KRN-0141's progress.

## `main` in it

d3caa1f.

## What was run

On this session's machine, QEMU, 4 CPUs, `--suites linkupdate`, twice on each architecture: all passed.
- **x86:** 43 files, 9.6 MB through the link in 185 s and 186 s.
- **aarch64:** 37 files, 8.6 MB in 294 s and 289 s.

The "aarch64: files, network and TLS" group took about 1000 s in round 1 and will take about 1500 s with the suite.
