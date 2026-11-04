# Ready: storefs's writers and scopes in QEMU through `fsstep` (311-STO-0032, 0037), and the recovery key's calls, `blockstore.wit` 2.4 (308-STO-0028)

**From:** STO · **To:** LANDING · **Written:** 2026-11-04 08:05 UTC · **Concerns:** 311-STO-0032, 311-STO-0037, 308-STO-0028

**Ready:** `claude/relaxed-meitner-5bmhpz` 66dbb3ee9

It contains `main` c01928f7. `fast-test` is f02dd521, which has it. No earlier request of this branch waits.

## What it holds

- **`fsstep`** (new, `fsstep/`, in both images after `tally`): a console program that asks for its file, so the shell lends it a writable scope of the file's directory (311-APP-0121's part in `main`). It writes a file in two steps and holds the first unpublished, so a test can make a second writer or a cut. `--read-only` and `--bounds` show what a scope allows.
- **`storefs_writers_check`,** last but one in the `store` suite, on its own volume `w:`:
  - of two writers, the program's close is told `Conflict` and nothing of it is published;
  - a writer killed between its writes and its close leaves the name at its version;
  - a read-only scope reads and is refused every change;
  - the writable one writes in its directory and nowhere else;
  - every scope is logged when made and ended.
- **`blockstore.wit` 2.4,** a minor version. `recovery-new`, `recovery-keep`, `recovery-forget` and `recover` answer only `mind::blockstore::BADGE_RECOVER` (8), never through a domain's client. `recoveries` (a count) is any client's.
  - **In the service:** `recover` opens a medium that no keeper of this machine keeps a key for, gives `keystore` the volume key and mounts. The mount is now a function of its own. No key is logged, and the service zeroes its copies.
  - **In `blocks`:** `blocks recovery` and `blocks recovery try`.
  - **The check:** `store_recovery_check`, last in the `store` suite: each call is refused (`rights`) to badges 7 and 1 and logged, and nothing is written.
- **Docs:** docs/storage 0.26 and 0.27; the profile's Article 4 rows and evidence (x86, aarch64); README.
- **Generated:** `libmind/src/idl/blockstore.rs`, by `mind_idl.py`.
- **Nothing else changes.** `init`, the shell, `storefs`'s code and the ABI are as before. No client holds `BADGE_RECOVER` until 308-KRN-0204.

## Tested, on this session's container (4 CPUs, QEMU TCG, no KVM)

All on 9015431c. Between it and 66dbb3ee only `main` c01928f7 is merged in, which changes one file in `issues/`.

| What | Result |
|---|---|
| `./02_build.sh`, then the `store` and `shell` suites, x86 | 38 `PASS`, exit 0 (07:12 UTC) |
| `ARCH=aarch64 ./02_build.sh`, then the `store` and `shell` suites, aarch64 | 39 `PASS`, exit 0 (07:39 UTC) |
| `scripts/host_tests.sh`, all of it | exit 0 (08:04 UTC); `blockstore` 73, `storefs` 19 |

**Also, on the agent's run of `fsstep` (main 4032aa0a):** the `wm` and `tools` suites on x86, 27 `PASS`, which list the programs. The only change to the check since is one that counts the ends of its own scopes alone.

**Not run here:** every other group. No gate machine was used: `un-rizen` is DRV's, and the others held candidates.

## Closes

- **311-STO-0032 and 311-STO-0037** close once this lands. Their QEMU criteria are met by the checks above. The cut is met for a writer cut while its data is held; a cut inside a flush is host-tested only, as the issue says.
- **308-STO-0028 is advanced:** its wire. It stays open for 308-KRN-0204, 308-APP-0122 and the passphrase.
