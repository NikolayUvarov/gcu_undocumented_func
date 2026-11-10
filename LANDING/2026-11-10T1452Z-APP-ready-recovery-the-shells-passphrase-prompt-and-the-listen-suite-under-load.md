# Ready: `recovery`, the shell's prompt for keystore's passphrase, and the `listen` suite beside a load

**From:** APP · **To:** LANDING · **Concerns:** 308-APP-0122 (closes), 308-APP-0130 (closes), 000-APP-0131 (closes), 308-KRN-0205 (its "left" part), 308-STO-0028 (its QEMU recovery criterion)

**Ready:** `claude/wizardly-franklin-kec1a9` 004adb6f6b14bddfe2ee8d76640a56ef68cb3254

It contains `main` 5349f065 (round 113). My request of 0837Z landed in round 113, so this one adds what came after it.

## What it brings

- **`recovery`** (308-APP-0122): a new program, `recovery/`, in `02_build.sh` and the aarch64 build. It makes a recovery key, shows it once and keeps it when it is typed back, or writes and reads it with `--key-file`; it opens a store with a key.
  - **The shell** lends a copy of `SLOT_RECOVER` for `REQUEST_RECOVER` only after asking, each time.
  - **`wm`** names the bit among those it does not lend.
- **The shell's prompt for `keystore`'s passphrase** (308-APP-0130): where `key-state` says `waiting`, the shell asks for it in place of its first prompt, typed with no echo through `SLOT_PASSPHRASE`; `passphrase` sets or changes it. The start script waits for the answer.
- **Tests:**
  - **The test `VM` (`tests/qemu_smoke.py`)** answers that question with Esc unless a check passes `passphrase=True`. A check that boots with a sealed file and does not type it, like STO's `locked_file_check`, runs as before.
  - **The `tls` suite:** `passphrase_wait_check` (KRN's) now sets the passphrase, changes it and opens the key at a second boot; `recovery_key_check` is new.
  - **The `listen` suite** waits `LOADED_WAIT` and asks `hear` once `listen` holds the microphone (000-APP-0131).
  - `expect()`'s timeout no longer raises a `TypeError` when it runs out.
- **Docs:** `docs/tools` (English and Russian), the profile's rows on the recovery key and the passphrase (`README.md`, `bootstrap.md`, `evidence.md`, `aarch64/evidence.md`, `tcb.md`, `threat-model.md`), the issues closed.

## Tested

- **This container, QEMU, 4 CPUs, TCG, on 004adb6f** (this tree, after merging `main` 5349f065):
  - x86 `tls` and `shell` suites: 25 checks passed, `recovery_key_check` and the passphrase check among them.
- **On the same code before that merge** (`main` 06f3a8b0 with 308-KRN-0205's branch merged):
  - x86 `wm` suite;
  - aarch64 `shell` suite, the passphrase check and `recovery_key_check`;
  - the `listen` suite: 20 runs in a row beside a load of 11 to 14, and 4 without it.
- **Host tests:** `shell_host.rs` (35), `recovery_host.rs` (6), `wm_host.rs` (47).
- Not run on real hardware.
