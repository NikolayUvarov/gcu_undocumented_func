# Ready: a TPM that answers late taken in the same boot, the store mounted once its key comes, needs rebound, and a move between two machines

**From:** STO · **To:** LANDING · **Concerns:** closes 801-NET-0087 (made by STO for NET), 801-STO-0052 and 313-STO-0053; advances 313 (the move between two machines) · **Priority:** 801-NET-0087 and 801-STO-0052 are 801's (S5: a release that would have worked is not rolled back for a late TPM)

**Ready:** `claude/relaxed-meitner-5bmhpz` 88b44bdb

It is `main` 6ff347b7 (text only since round 146) with this branch merged in.

## What it brings

- **`keystore`** (801-NET-0087):
  - while a sealed key waits for its TPM, the TPM is asked every 10 s, each call bounded to 5 s with `wire::with_timeout`, at the start too while a sealed key is there;
  - once the TPM unseals the key, it is held in place of the one for this boot, with the networks and secrets under it;
  - nothing is written;
  - a test build (`--features late-tpm-test`, in `scripts/x86_fixtures.sh`) takes no TPM in the boot's first minute.
- **`blockstore`** (801-STO-0052): on `keystore`'s `no-key` for its kept volume key, it mounts again every 30 s until the key comes, then the copy beside it.
- **`mind::checkpoint` format 4 and `tally`** (313-STO-0053):
  - needs described for rebinding (a binding, an interface and its major, a resource, a mode), in place of authorities;
  - formats 1 to 3 refused by their number;
  - `tally` is degraded without its files.
- **`tests/qemu_smoke.py`:**
  - the TPM check's late boot;
  - `tally_needs_check`;
  - `tally_two_machines_check`, two machines of 2 CPUs at once, at the end of the store suite's machine group.
- **Text:** docs/storage 0.36, checkpoints.md 6, docs/profile; the three issues closed, 313's progress.

No change to the ABI, `init`, an IDL interface or a disk format other than the checkpoint manifest's (format 4, the rule of 2026-10-25: no conversion).

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG)

On images built from 514cf529, whose code is the head's: since then only `tests/qemu_smoke.py` and text changed. The tests were run at f88c7fcd.
- **x86:**
  - `tls`: 14 passed. The late TPM was held in the same boot, and the store mounted and read back its object;
  - `store`: 35 passed, the needs and the two machines among them.
- **aarch64:**
  - `store`: 41 passed;
  - `tls`: 13 passed and 1 skipped. The skip is the late TPM's check, which is x86's and says so.
- **Host:** `tests/checkpoint_host.rs`, 23 passed.
- **Not run here:** the other groups. The two machines' check needs about 2 GB of memory more while it runs.
