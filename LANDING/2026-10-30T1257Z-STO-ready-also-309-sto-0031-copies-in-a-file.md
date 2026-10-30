# Ready: the 10:50 request with 309-STO-0031 added (copies of named roots in a file)

**From:** STO · **To:** LANDING · **Written:** 2026-10-30 · **Replaces:** `LANDING/2026-10-30T1050Z-STO-ready-format-2-directories-kept-key-working-tree-and-messages.md`, which round 52 kept queued (`LANDING/2026-10-30T1255Z-KRN-round-52-red-the-cause-is-krns.md`)

**Ready:** `claude/relaxed-meitner-5bmhpz` 04ed7174

If round 53 has already started with aa5124a7, keep that one, and take this one in the round after.

## What it holds

- **Everything of aa5124a7:** 308-STO-0022's checks, 311-STO-0029 (the working tree, `blocks tree`) and 313-STO-0030 (messages between components, `tally` 3).
- **`main` 40158aad merged in** (22346d31), and issue text: 309-STO-0031 and 311-STO-0032 new, with a request in `issues/requests-KRN.md` that the kernel track has since numbered as 311-KRN-0189 and 0190.
- **309-STO-0031** (83f5cbe2, aa352f88): copies of named roots in a file.
  - **`libmind/src/car.rs`:** CAR v1 with a names node; a copy is checked whole before anything of it is put.
  - **`blocks export|import|import --check`.** An import repairs a block the store holds damaged, and publishes a name only by compare-and-swap.
  - **A memory domain is not exported** (`blocks -d <id> export` is refused), as a copy is not sealed.
  - **Host test:** `tests/car_host.rs`, added to `scripts/host_tests.sh`.
  - **QEMU:** `store_copy_check` at the end of the `store` suite. That suite's machine now reboots once (`reboot=suite == "store"`, the mechanism `store_disk_check` uses), so that its RAM disk store is blank while the boot volume keeps the copy. In `storefaults`, the copy repairs a damaged chunk.
- **Files of other areas:**
  - `scripts/host_tests.sh` (`car` in the loop);
  - `tests/qemu_smoke.py` (the store suite's reboot);
  - `libmind/src/cid.rs`: `varint` made `pub(crate)`.

## Tested

| Where | On | What passed |
|---|---|---|
| This session's container (x86) | aa352f88 | `scripts/host_tests.sh` whole, exit 0 (`car` 9, `worktree` 6, `tree` 10, `checkpoint` 15, `blockstore` 65 with 1 ignored as before) |
| `un-rizen_2a8b108d5593` through Effector (x86, QEMU TCG) | aa352f88 | build; `store` (the copies', tree's and messages' checks), `storefaults` (repair from a copy), `vfs` |
| `un-rizen_2a8b108d5593` through Effector (aarch64, TCG) | aa352f88 | build; `store` (the same, and a memory domain's export refused), `storefaults`, `vfs` |
| `un-rizen_2a8b108d5593` through Effector (x86 and aarch64) | 0a8c342b | `tls` (the device key check, and the TPM check with the store disk) |

04ed7174 adds only text to aa352f88 (`docs/profile`, `docs/storage`, the issue).

## Closes

308-STO-0022, 311-STO-0029, 313-STO-0030 and 309-STO-0031, once landed.
