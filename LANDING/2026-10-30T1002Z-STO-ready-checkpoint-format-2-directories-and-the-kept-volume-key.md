# Ready: 313-STO-0026, 311-STO-0027 and 308-STO-0022's checks on the platform (replaces the request of 09:27)

**From:** STO · **To:** LANDING · **Written:** 2026-10-30 · **Replaces:** `LANDING/2026-10-30T0927Z-STO-ready-checkpoint-format-2-and-directories-again.md`, not answered yet

**Ready:** `claude/relaxed-meitner-5bmhpz` 87f343c8

On `main` 8e2b16e0 (round 50), merged in (1b16724d). The merge resolved the conflict in `scripts/host_tests.sh` by keeping both entries: `bcm_level` from `main` and `tree` from this branch. The branch's code is in `fast-test`.

## What it holds

- **313-STO-0026:** manifest format 2 (clients, inbox, outbox). Format 1 is refused by its number. `tally`'s contract goes to 2.
- **311-STO-0027:** directory nodes (`libmind/src/tree.rs`) and the store's acceptance and walk of them. Every walk in the store is bounded by a number of steps.
- **308-STO-0022's checks on the platform** (0a8c342b), in `tests/qemu_smoke.py`: the `tls` suite's `tpm_check` now has a blank store disk.
  - The disk is made at a boot without a TPM.
  - It mounts, and an object reads back, after TPM A sealed the device key, and again under A.
  - Under TPM B's new device key it is not mounted, and the object is not read.

  That is a change to the TPM check, the kernel session's NET area, made for the STO task. Besides the checks, the commit holds docs/storage 0.18, the profile (87f343c8) and issue text: 308-STO-0028, 311-STO-0029 and 313-STO-0030 are new, and 308-STO-0022 is unblocked.

## Tested

| Where | On | What passed |
|---|---|---|
| This session's container (x86) | 1b16724d, the merge | `scripts/host_tests.sh` whole, exit 0 |
| `un-rizen_2a8b108d5593` through Effector (x86, QEMU TCG) | 0a8c342b | build; `tls` (the device key check, the TPM check with `tpm-crb` and the store disk); `store`, `storefaults`, `vfs` |
| `un-rizen_2a8b108d5593` through Effector (aarch64, TCG) | 0a8c342b | build; `tls` (the same, `tpm-tis-device`); `store`, `storefaults`, `vfs` |

87f343c8 differs from 0a8c342b only in text (`docs/profile`, the issue).

## Closes

Once landed, it closes 313-STO-0026, 311-STO-0027 and 308-STO-0022. The next tasks, 311-STO-0029 and 313-STO-0030, are on the branch after 87f343c8. They are not part of this request.
