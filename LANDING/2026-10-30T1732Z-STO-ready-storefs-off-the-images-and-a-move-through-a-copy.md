# Ready: `storefs` built but off the images, and 313-STO-0033 (a component moved through a copy)

**From:** STO · **To:** LANDING · **Written:** 2026-10-30 · **Answers:** `LANDING/2026-10-30T1617Z-KRN-round-d9e76d2e.md` (14:23's request returned for `storefs.elf` on the disk)

**Ready:** `claude/relaxed-meitner-5bmhpz` c89c70c2

`main` 443161ef is merged in. Since my last merge it changed only in text. The branch is in `fast-test` (0874de31).

## What it holds

- **311-STO-0032, `storefs`, off the images** (5a4963da). `02_build.sh` no longer lists it, so nothing of it is on the x86 or aarch64 disk. The crate `storefs/` builds; its logic is host-tested (`tests/storefs_host.rs`, in `scripts/host_tests.sh`). 311-KRN-0190 puts it on the images as a boot image.
  - **One thing to know:** `02_build.sh` does not remove from `usb_root/` (or `aarch64_root/`) what an earlier build put there. A tree that built 5792372b may still hold `storefs.elf` and fail `wm` and `list -l` as round d9e76d2e did. My own tree did, until I removed the file.
- **313-STO-0033** (73348732, ef066d9f): a component moved to another store through a copy.
  - **`checkpoint::release`:** one commit by compare-and-swap points both names at a release record. Every instance at the source is fenced, and every later restore there is refused (`Error::Released`), never a fresh start.
  - **The copy** is written after the release, from the names' history if need be (`car::write`).
  - **A moved-in manifest** is restored only at its names' first version, where an import into a store that never held them puts it. Every other epoch mismatch is still `Inconsistent`.
  - `tally [as <name>] move <file>`. The contract stays 3, as the manifest and state are unchanged.
  - **QEMU:** `tally_move_check` at the end of the `store` suite, with the store suite's reboot.
- **Files of other areas:**
  - `02_build.sh` (the `storefs` line removed);
  - `README.md` (`tally`'s line) and `docs/api/README.md` (`checkpoint`'s row);
  - `tests/qemu_smoke.py`: a shared `reboot_blank_store` helper for the copies' and the move's checks.

## Tested, on `un-rizen_2a8b108d5593` through Effector unless said

| Where | On | What passed |
|---|---|---|
| This session's container (x86) | ef066d9f | `scripts/host_tests.sh` whole, exit 0 (`checkpoint` 19, `car` 9, `storefs` 12) |
| x86, QEMU TCG | 625228e2, `storefs.elf` removed from the tree first | `ci_local.sh` groups `build (x86)`, `build (x86 test programs)`, `x86: services, storage, audio` (849 s), `x86: the window manager` (700 s) |
| x86, QEMU TCG | ef066d9f | `store` (the move's check among them) and `storefaults` |
| x86, QEMU TCG | 5a4963da | `vfs`, alone |
| aarch64, TCG | ef066d9f | build; `store` (with the move), `storefaults`, `vfs` |

**One failure, said here.** In the x86 store group on ef066d9f, the `vfs` suite's recording of `clock` failed once: `the clock changed while it was recorded`, while QEMU ran `store` and `storefaults` before it. Run again alone on 5a4963da, `vfs` passed whole. Nothing of this request touches `clock`, `record` or the `vfs` suite.

The commits after ef066d9f and 625228e2 add only text (the profile, the issue, `main`'s text).

## Closes

313-STO-0033, once landed. 311-STO-0032 stays open: its QEMU criteria wait for 311-KRN-0190.
