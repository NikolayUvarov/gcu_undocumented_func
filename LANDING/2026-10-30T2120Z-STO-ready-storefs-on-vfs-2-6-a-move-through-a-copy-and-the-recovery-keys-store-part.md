# Ready: `storefs` on `vfs.wit` 2.6 and off the images, 313-STO-0033, and 308-STO-0028's store part (host-tested)

**From:** STO · **To:** LANDING · **Written:** 2026-10-30 21:20 UTC · **Replaces:** `LANDING/2026-10-30T1952Z-STO-ready-the-same-with-both-host-tests-names.md` (withdrawn at 20:45)

**Ready:** `claude/relaxed-meitner-5bmhpz` 622dadc7

`main` ad3f64ff is merged (a3d171b7) and holds nothing newer. `fast-test` is ca23bdf2.

## What it holds

- **311-STO-0032, `storefs`:**
  - off the images until 311-KRN-0190 (5a4963da);
  - now on `vfs.wit` 2.6 (622dadc7): a conflict is `conflict`, not `io`, and `tests/storefs_host.rs` checks the mapping against the generated 2.6 enum. That check of 2.5 is what made me withdraw 1952Z.
- **313-STO-0033,** a component moved to another store through a copy (73348732, ef066d9f), as in 1732Z.
- **308-STO-0028, the store's part of the recovery key** (203d51de). This is new since 1952Z.
  - **What it adds:** `blockstore/src/store.rs` gains a recovery record and a two-step rotation, and `libmind/src/blockstore.rs` the key's text form.
  - **Nothing on the wire changes:** `blockstore.wit`, `blockstore/src/main.rs` and `blocks` are as before.
  - **It is reachable only from the host tests.** The service builds it but calls none of it.
- **Files of other areas:**
  - `02_build.sh` (the `storefs` line removed);
  - `README.md` (`tally`'s line) and `docs/api/README.md` (`checkpoint`'s row);
  - `tests/qemu_smoke.py`: a shared `reboot_blank_store` helper.

**The note on a stale `usb_root/storefs.elf` still holds.** `02_build.sh` does not remove it, and a tree that built 5792372b may still carry it.

## Tested

| Where | On | What passed |
|---|---|---|
| This session's container (x86) | 622dadc7 | `./02_build.sh` (stale `storefs.elf` removed first), then `scripts/host_tests.sh` whole, exit 0. Among it: `storefs` 12 on 2.6, `blockstore` 73 with the recovery tests (built `-O -C debug-assertions=on`), `checkpoint` 19, `car` 9 |
| This session's container | 203d51de | `blockstore_host` also built without `-O`; `cargo build --release` of `blockstore` (x86); the agent's run of it for aarch64 |
| `un-rizen_2a8b108d5593`, x86, QEMU TCG | 625228e2 | `ci_local.sh` groups `build (x86)`, `build (x86 test programs)`, `x86: services, storage, audio` (849 s), `x86: the window manager` (700 s) |
| `un-rizen_2a8b108d5593`, x86, QEMU TCG | ef066d9f | `store` (with the move's check) and `storefaults` |
| `un-rizen_2a8b108d5593`, x86, QEMU TCG | 5a4963da | `vfs`, alone |
| `un-rizen_2a8b108d5593`, aarch64, TCG | ef066d9f | build; `store` (with the move), `storefaults`, `vfs` |

**The QEMU runs predate both merges of `main`, the recovery key and the 2.6 mapping.**
- The recovery key's code is in `store.rs`, which the image's `blockstore` builds, but it is called only by host tests.
- The 2.6 mapping is in `storefs`, which is not on the images.
- 1732Z's note on the one failed `vfs` clock recording, passed when run again alone, stands.

## Closes

- 313-STO-0033, once landed.
- 311-STO-0032 stays open until 311-KRN-0190; 308-STO-0028 until the wire is agreed with APP and KRN.
