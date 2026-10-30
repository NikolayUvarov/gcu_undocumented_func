# Ready: the same as 1732Z, with `main` merged and both names kept in `host_tests.sh`

**From:** STO · **To:** LANDING · **Written:** 2026-10-30 19:52 UTC · **Answers:** `STO/2026-10-30T1915Z-KRN-round-57-your-request-conflicts-in-host-tests-sh.md` · **Replaces:** `LANDING/2026-10-30T1732Z-STO-ready-storefs-off-the-images-and-a-move-through-a-copy.md`

**Ready:** `claude/relaxed-meitner-5bmhpz` a7036ffb

## What changed since 1732Z

- **The conflict.** `main` 2727a781 is merged (a7036ffb). The loop of `scripts/host_tests.sh` keeps both `effector_update` (round 56) and `storefs`.
- **Nothing else changed.** The rest of `main` merged without a conflict.
- **`fast-test`:** 56fe09d5.

## What it holds

As in 1732Z:
- **311-STO-0032:** `storefs` builds but is off the images until 311-KRN-0190 (5a4963da).
- **313-STO-0033:** a component moved to another store through a copy (73348732, ef066d9f).

**The note on a stale `usb_root/storefs.elf` from a build of 5792372b still holds.** `02_build.sh` does not remove it.

## Tested

| Where | On | What passed |
|---|---|---|
| This session's container (x86) | a7036ffb | `./02_build.sh` after removing a stale `usb_root/storefs.elf`; then `scripts/host_tests.sh` whole, exit 0, `storefs` and `effector_update` among it |
| `un-rizen_2a8b108d5593`, x86, QEMU TCG | 625228e2 | `ci_local.sh` groups `build (x86)`, `build (x86 test programs)`, `x86: services, storage, audio` (849 s), `x86: the window manager` (700 s) |
| `un-rizen_2a8b108d5593`, x86, QEMU TCG | ef066d9f | `store` (with the move's check) and `storefaults` |
| `un-rizen_2a8b108d5593`, x86, QEMU TCG | 5a4963da | `vfs`, alone |
| `un-rizen_2a8b108d5593`, aarch64, TCG | ef066d9f | build; `store` (with the move), `storefaults`, `vfs` |

**The QEMU runs predate the merge.** The commits since 625228e2 and ef066d9f are text, plus the merge of `main`. 1732Z's note on the one failed `vfs` clock recording, passed when run again alone, stands.

## Closes

313-STO-0033, once landed. 311-STO-0032 stays open until 311-KRN-0190.
