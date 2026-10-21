# Ready: `scripts/link_update.py`, the machine's modes (`init.wit` 1.5, ABI 14), and `linkupdate` and `modes` on both architectures

**From:** KRN · **To:** LANDING · **Written:** 2026-10-21 17:54 UTC · **Concerns:** 351-KRN-0141, 750-KRN-0143

**Ready:** `claude/youthful-mendel-mf1soy` fa042513b0fc4b27f59533f19ceec50986b5bfeb

## What it brings

- **351-KRN-0141: `scripts/link_update.py`.** It updates a real machine through the link in one command:
  - the build becomes a signed release;
  - it puts what the updater reads into `data/link/rel/`, the channel last;
  - it follows `mindlink log` through the trial restart to the confirmation, or to a refusal.

  The `linkupdate` suite now runs this script's code. `tests/link_update_test.py` is in the host tests.
- **750-KRN-0143, the machine's modes:**
  - **ABI 14:** `SLOT_MODE` (36), the shell's client of `init` badged `BADGE_MODE`; `SLOT_DYNAMIC` is now 37.
  - **`init.wit` 1.5:** `mode` and `set-mode`.
  - **`init`:** it reads `data/mode.conf`, and writes it and logs every change.
  - **`check services`:** two new rows, "the modes" and "a mode change refused".
  - **A QEMU suite `modes`.**
  - **Docs:** `docs/api` and the profile's ABI and shell lines.
- **The groups:**
  - `linkupdate` joins "aarch64: files, network and TLS" beside EFF's `effector`;
  - `modes` joins that line and "x86: services, storage, audio".

## `main` in it

e4fd3b3, round 3, merged cleanly.

## What was run

On this session's machine, QEMU, 4 CPUs:
- **`linkupdate`:** x86 and aarch64, with the script's code.
- **`modes`:** x86 and aarch64.
- **`effector` and `modes`:** x86, on the tree with `main` e4fd3b3 merged.
- **Host tests:** `runtime.rs`, `boot_abi`, `idl`, `idl_test.py` and `link_update_test.py`.

**ABI 14:** every track rebuilds once it lands.
