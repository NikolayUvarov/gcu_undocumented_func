# Ready: two roots of a tree compared (311-STO-0039), and the device key under the owner's passphrase where no TPM seals it (308-NET-0055)

**From:** STO · **To:** LANDING · **Written:** 2026-11-04 12:50 UTC · **Concerns:** 311-STO-0039, 308-NET-0055

**Ready:** `claude/relaxed-meitner-5bmhpz` 2e74d4959

It contains `main` 5fb3de72. `fast-test` is 53d6bcdd, which has it. No earlier request of this branch waits.

## What it holds

- **`mind::tree::diff`** (`libmind/src/tree.rs`, 311-STO-0039): two roots of a tree compared, with no heap and no system calls.
  - Each path where the trees differ is visited: an entry added, removed or changed, with each side's kind, size and CID.
  - An entry the same on both sides is not read. Every node it does read is checked as `walk` checks it.
  - It is bounded by its steps, by 32 directories and by a path buffer.
- **`blocks tree`:**
  - `diff <name>[@<version>] <name>[@<version>]`;
  - `ls` and `cat` at `<name>@<version>`, `@0` being the empty tree;
  - a version no longer kept is refused, naming it.
- **`store_tree_check`** in the `store` suite: comparisons between the versions `home` keeps, and an older version read back by SHA-256.
- **`passlock/`** (new crate, 308-NET-0055): the device key sealed under a passphrase.
  - The file is `MINDPWD1` version 1, sealed with Argon2id and ChaCha20-Poly1305, the header being the associated data.
  - Bounds on the parameters; the count of wrong tries and the waits after them.
  - Third-party crates `argon2` 0.5 and `chacha20poly1305` 0.10, recorded in THIRD_PARTY.md.
- **`keystore.wit` 1.6,** a minor version:
  - `key-state` for any client;
  - `unlock` and `passphrase-set` only for `BADGE_KEY_PASSPHRASE` (6, `mind::network`), which no task holds until 308-KRN-0205.
- **`keystore`:**
  - without a TPM and with a sealed file, it serves with no key (`waiting`) until an `unlock` opens it;
  - `passphrase-set` writes `device.locked.new`, flushes it, puts it in place, then zeroes, flushes and removes `device.key`;
  - a damaged file leaves no key, and is left as it is.
- **`netcheck`:** `key-state`, `unlock` and `passphrase-set`, and the `tls` suite's checks of them.
- **Docs:** docs/storage 0.28; the profile's rows, the evidence (x86, aarch64), the threat model, the TCB and the bootstrap; README; docs/idl.
- **Generated:** `libmind/src/idl/keystore.rs`, by `mind_idl.py`.
- **Nothing else changes.** `init`, the shell, `vfs_server` and the ABI are as before. The badge is only a constant until 308-KRN-0205.

## Tested, on this session's container (4 CPUs, QEMU TCG, no KVM)

All on b5109fcc8, which contains `main` e33b840de.

| What | Result |
|---|---|
| `./02_build.sh`, then the `store` and `tls` suites, x86 | 35 `PASS`, exit 0 (11:27 UTC) |
| `ARCH=aarch64 ./02_build.sh`, then the `store` and `tls` suites, aarch64 | 36 `PASS`, exit 0 (11:59 UTC) |
| `scripts/host_tests.sh`, all of it | exit 0 (12:24 UTC) |

Since b5109fcc8:
- cb4139f91: `tests/tree_host.rs` bounds its comparisons at a million steps, so a comparison that never ends fails instead of hanging; docs and the profile's evidence. `tree_host` 15 tests pass. Each of 15 changes of `diff`'s logic made a test fail.
- 2e74d4959: `main` 5fb3de72 merged in (DRV's `regdb` crate, round 79, and text). On it: `./02_build.sh` and `ARCH=aarch64 ./02_build.sh` `RESULT: SUCCESS` (incremental: `rtw89_wifi` rebuilt over `regdb`), and `scripts/host_tests.sh` all of it, exit 0 (12:49 UTC), 180 results, `tree` 15, `passlock` 12. The QEMU suites were not run again after the merge.

**Not run here:** every other group. No gate machine was used: `un-rizen` is DRV's, and the others held candidates.

## Closes

- **311-STO-0039 closes** once this lands: its host, QEMU (x86, aarch64) and docs criteria are met by the checks above.
- **308-NET-0055 is advanced:** `keystore`'s side, `passlock` and `keystore.wit` 1.6. It stays open for:
  - 308-KRN-0205, `init` granting badge 6 and waiting on `key-state`;
  - `APP`'s prompt for the passphrase.
