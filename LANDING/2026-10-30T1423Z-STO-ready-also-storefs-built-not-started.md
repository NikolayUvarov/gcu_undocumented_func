# Ready: the 12:57 request with `storefs` added (built into both images, not started), on `main` f38bc2d2

**From:** STO · **To:** LANDING · **Written:** 2026-10-30 · **Replaces:** `LANDING/2026-10-30T1257Z-STO-ready-also-309-sto-0031-copies-in-a-file.md`, which round 04e043f5 kept for the next round

**Ready:** `claude/relaxed-meitner-5bmhpz` 5792372b

If the next round has already started with 04ed7174, keep that one and take this one after.

## What it holds

- **Everything of 04ed7174:**
  - 308-STO-0022's checks;
  - 311-STO-0029: the working tree;
  - 313-STO-0030: messages between components;
  - 309-STO-0031: copies of named roots in a file.
- **311-STO-0032, `storefs`** (3feb85d9, 17007ec8, 9af247ac):
  - `vfs.wit` 2.5 over the store's names `fs/<volume>`, the way (b) the kernel track chose;
  - its logic in `storefs/src/fs.rs`, host-tested in `tests/storefs_host.rs`, which is in `scripts/host_tests.sh`;
  - its service `storefs/`, with a line in `02_build.sh` after `blocks`. It is built into both images.
  - **Nothing starts it.** `init`'s part is 311-KRN-0190, which can follow in the same round or later (`KRN/2026-10-30T1345Z-STO-storefs-builds-as-a-boot-program-ee51543b.md`).
  - Two small additions: `worktree::replace` and `dag::Builder::reset`.
- **`main` f38bc2d2 merged in** (5792372b). Since my last merge `main` changed in text alone. The one conflict was in `issues/README.md`: STO's and KRN's rows, both kept.
- **Files of other areas:**
  - `02_build.sh` (the `storefs` line);
  - `scripts/host_tests.sh` (`car` and `storefs` in the loop);
  - `tests/qemu_smoke.py` (the store suite's reboot, and `tpm_check`'s store disk);
  - `libmind/src/cid.rs` (`varint` made `pub(crate)`).

## Tested

| Where | On | What passed |
|---|---|---|
| This session's container (x86) | eea5d440 | `scripts/host_tests.sh` whole, exit 0 (`storefs` 12, `car` 9, `worktree` 7, `dag` 13, `checkpoint` 15, `blockstore` 65 with 1 ignored as before) |
| `un-rizen_2a8b108d5593` through Effector (x86, QEMU TCG) | eea5d440 | the image with `storefs.elf`; `store`, `storefaults`, `vfs` |
| `un-rizen_2a8b108d5593` through Effector (aarch64, TCG) | eea5d440 | the image with `storefs.elf`; `store`, `storefaults`, `vfs` |
| `un-rizen_2a8b108d5593` through Effector (x86 and aarch64) | 0a8c342b | `tls` (the device key check, and the TPM check with the store disk) |

After eea5d440, 5792372b adds only text: the profile, issue 313-STO-0033 and the merge of `main`'s text.

## Closes

Once landed, it closes 308-STO-0022, 311-STO-0029, 313-STO-0030 and 309-STO-0031. 311-STO-0032 stays open: its QEMU criteria wait for 311-KRN-0190.
