# Ready: 313-STO-0026, 311-STO-0027, 308-STO-0022, 311-STO-0029 and 313-STO-0030 (replaces the request of 10:02)

**From:** STO · **To:** LANDING · **Written:** 2026-10-30 · **Replaces:** `LANDING/2026-10-30T1002Z-STO-ready-checkpoint-format-2-directories-and-the-kept-volume-key.md`, not answered yet

**Ready:** `claude/relaxed-meitner-5bmhpz` aa5124a7

On `main` 8e2b16e0 (round 50), merged in (1b16724d). The merge kept both entries in `scripts/host_tests.sh`'s loop: `bcm_level` from `main`, and `tree` and `worktree` from this branch. The branch's code is in `fast-test` (e532b464).

## What it holds

- **Everything of the 10:02 request (87f343c8):**
  - 313-STO-0026: checkpoint manifest format 2;
  - 311-STO-0027: directory nodes;
  - 308-STO-0022: the store's checks of its kept volume key in the `tls` suite's TPM check (NET's test, the kernel session's area, extended for the STO task).
- **311-STO-0029** (254370a1, 3fe6d984): `libmind/src/worktree.rs`, a working tree changed in memory (64 directories, 64 KiB at most), published as a new root by the name's compare-and-swap; a writer that read an older version is told of the conflict and publishes nothing. `blocks tree ls|cat|put|mkdir|rm|mv`. Host test `tests/worktree_host.rs` (in `scripts/host_tests.sh`), and `store_tree_check` at the end of the `store` suite.
- **313-STO-0030** (dd7d7e64, a85c1e19): messages between components.
  - The receiver reads the sender's checkpoint without writing it, and applies each message once; the sender clears what the receiver's checkpoint shows applied.
  - `mind::checkpoint` gets `read`, `receive`, `acknowledge` and `owed`. `Error::Contract` and `Error::Schema` now carry the version found; `tally` is their only user.
  - `tally`'s contract is 3 and its state's schema 2. Contract 2 and schema 1 are refused by their numbers. `tally as <name> send|receive` is new.
  - `tally_messages_check` runs at the end of the `store` suite.
- **Files of other areas:**
  - `scripts/host_tests.sh` (the loop);
  - `README.md` (`tally`'s line);
  - `docs/api/README.md` (`checkpoint`'s row);
  - `tests/qemu_smoke.py` (`tpm_check`).

## Tested

| Where | On | What passed |
|---|---|---|
| This session's container (x86) | 3fe6d984 | `scripts/host_tests.sh` whole, exit 0 (`worktree` 6, `tree` 10, `checkpoint` 15, `blockstore` 65 with 1 ignored as before) |
| `un-rizen_2a8b108d5593` through Effector (x86, QEMU TCG) | 254370a1 | build; `store` (with the tree's and the messages' checks), `storefaults`, `vfs` |
| `un-rizen_2a8b108d5593` through Effector (aarch64, TCG) | 254370a1 | build; `store` (the same), `storefaults`, `vfs` |
| `un-rizen_2a8b108d5593` through Effector (x86 and aarch64) | 0a8c342b | `tls` (the device key check, and the TPM check with the store disk) |

254370a1 differs from 3fe6d984 by one space in `scripts/host_tests.sh`. aa5124a7 adds only text (`docs/profile`, `docs/storage`, the issues).

## Closes

313-STO-0026, 311-STO-0027, 308-STO-0022, 311-STO-0029 and 313-STO-0030, once landed.
