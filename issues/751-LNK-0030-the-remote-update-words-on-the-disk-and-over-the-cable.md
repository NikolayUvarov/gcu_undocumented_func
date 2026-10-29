# 751-LNK-0030 — The words `remote-update` and `link-remote-update`: on the disk as it is written, and over the cable only if the owner allowed it

**Type:** tool, tests · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-10-29) · **Blocked by:** `KRN`: the words in `init` and `init.wit` 1.11; `APP`: the link's console taking `mode remote-update` only with `link-remote-update` (both asked on `internal-dev-messages`, 2026-10-29) · **Main task:** [751](751-system-updates-at-a-developers-request.md) · **Constitution:** MC-12.6, MC-7.5

## Problem

[751](751-system-updates-at-a-developers-request.md) adds the owner's permission `remote-update`, a word of `data/mode.conf`. It lets a developer agent update the system through Effector.
- **During active development** the maintainer's disks are written with the word on.
- **The cable** may switch it only where the owner allowed that with a second word, `link-remote-update`. That word is set at the console or on the disk, never by the cable.

## Plan

1. **`scripts/build_and_write.sh --mode=`:**
   - takes `remote-update` beside `development`, `maintenance` and `link-console`;
   - refuses it without `development` or `maintenance`, as `init` would not count it;
   - takes `link-remote-update` only with `--link-dev` and `link-console`, as `link-console` itself is taken (800-LNK-0019).
2. **`--keep-settings`** keeps the words as it keeps `data/mode.conf` now. An option given in the same write still wins.
3. **The `link` suite** (QEMU, x86):
   - **with `dev`, `link-console` and `link-remote-update`:** `sh mode remote-update on` and `off` are taken, and `init` logs the change with the link as its source;
   - **with `dev` and `link-console` alone:** the same lines are refused, as `mode` is today;
   - **in either case:** `mode link-remote-update on` from the link is refused.
4. **The docs:**
   - `docs/write-disk.md` (both languages): the words;
   - `docs/link/mindlink.md` (both languages): what the cable may switch and when.

## Progress

- **2026-10-29, the script's part:**
  - `--mode=` takes `remote-update` (with `development` or `maintenance`) and `link-remote-update` (with `link-console`, which still needs `--link-dev`), and refuses them otherwise;
  - `tests/build_and_write_args_test.py` checks the refusals with a stand-in `wslpath`, and no build or write starts;
  - `docs/write-disk.md` (both languages) describes the words, and that `init` leaves them out until `KRN`'s part lands.
- **Left:** the `link` suite's cases and the machine's run, after `KRN`'s and `APP`'s parts.

## Acceptance criteria

- [x] `build_and_write.sh`'s argument checks, run past parsing only (`tests/build_and_write_args_test.py`, 2026-10-29).
- [ ] The `link` suite's three cases above pass on x86.
- [ ] On the DEXP or the MacBook Pro, over the cable, with the owner's `link-remote-update`: `remote-update` turned on and off. Evidence for that machine only (MC-12.1).

## Related

[751](751-system-updates-at-a-developers-request.md), [800-LNK-0019](800-LNK-0019-the-link-as-the-owners-console.md), [800-APP-0108](../issues-done/800-APP-0108-the-links-lines-as-the-consoles.done).
