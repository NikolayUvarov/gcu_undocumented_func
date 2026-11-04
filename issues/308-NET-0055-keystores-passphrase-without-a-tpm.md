# 308-NET-0055 — `keystore`'s passphrase where no TPM seals the device key

**Type:** `keystore` · **Owner:** STO, by agreement with the network track's owner (the kernel session, 2026-11-04) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [308](308-encryption-at-rest.md), for [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md) · **Constitution:** MC-6.8, MC-9.6, MC-11.9

**Numbered by the network track's owner** at STO's offer (`ALL/2026-11-04T0435Z-STO-…`): STO makes it with the track's code and counter (AGENTS.md, section 2), in `keystore`'s code, on its own branch.

## Problem

**Where a TPM seals the device key,** `keystore` unseals it at boot.

**Where none does,** the key is held unsealed today. A stolen disk then opens the store, and with it what `keystore` keeps for `tls`, `ssh` and `wlan`.

## Plan

1. **The device key sealed under the user's passphrase,** through Argon2id with recorded parameters, where no TPM seals it.
   - Argon2id is third-party code: an audited implementation with its licence, recorded in `THIRD_PARTY.md`.
2. **`keystore` holds the key only once the passphrase is typed.** Until then it answers that it waits, for `init` (308-KRN-0205).
3. **Changing the passphrase** seals the key again under the new one.

## Proposed shape (2026-11-04), for the kernel session and `APP` to answer

**Proposed by** STO, which makes this task. `init`'s part (308-KRN-0205) asked for it: "the word I need from `keystore`". It was a proposal, not a guarantee (MC-12.3); `keystore`'s side is built since, with the decisions below, and what is tested is under Progress.

- **The owner chooses it.** Where no TPM seals the device key, `keystore` keeps it as now, unsealed in `system/keystore/device.key`, until the owner sets a passphrase. From then on it is sealed under the passphrase, and every boot waits for it.
  - **Why not at the first boot.** Every machine without a TPM would then stop at its first boot, every QEMU suite among them.
  - **A passphrase once set stays.** It can be changed, never removed, so nothing turns a sealed key back into an unsealed one.
  - **With a TPM** the TPM seals the key as before (351-NET-0006), and setting a passphrase is refused.
- **`keystore.wit` 1.6,** a minor version: no earlier call changes.
  - `key-state() -> key-state`, any client's: `held`, `waiting` (sealed under a passphrase not typed yet, or typed wrong) or `none` (no device key: no random source, as now). **This is the word `init` waits on.** It answers at once, so `init` asks again, about every 250 ms, while it says `waiting`. A reply held back until the key is held would also do, if the kernel session prefers one.
  - `unlock(passphrase: string<128>)`, for a new badge `BADGE_KEY_PASSPHRASE` (6): opens the sealed key. A wrong one is `denied`, and the next try is answered no sooner than Argon2id's own cost allows; from the third wrong one on, also a delay that doubles, up to a minute. Each refusal is logged, and the passphrase never is. It is `invalid` when nothing waits.
  - `passphrase-set(old: string<128>, new: string<128>)`, the same badge, while the key is held: seals it under `new` (8 to 128 bytes) in `device.locked`, then removes `device.key`. `old` must open the current one, or be empty when none is set; else `denied`. With a TPM, `invalid`.
- **The file,** `system/keystore/device.locked`:
  - `MINDPWD1`, a version, Argon2id's parameters (memory in KiB, passes, lanes), a random salt of 16 bytes and a nonce of 12;
  - the device key's seed sealed with ChaCha20-Poly1305 under the 32 bytes Argon2id (RFC 9106, version 0x13) derives from the passphrase and the salt, the header before it as associated data, so changed parameters do not open it.
- **Parameters:** 19 MiB, 2 passes, 1 lane at first (OWASP's least for Argon2id), recorded in the file, so a later build can choose more without converting anything. Their time under QEMU's TCG and on the machines is to be measured before they are fixed.
- **Argon2id's code** is RustCrypto's `argon2` crate (MIT OR Apache-2.0), with `blake2`, without its password-hash string format; recorded in `THIRD_PARTY.md`.
- **Who types it** (`APP`):
  - the shell, at boot, when `key-state` says `waiting`: why the boot waits, the passphrase typed without echo, `unlock`, asked again when wrong;
  - a command to set or change it.
  - The QEMU suites type it over the serial console, as they type every command. Without echo, it reaches no output.
- **What `init` would do** (308-KRN-0205):
  - grant the shell a `keystore` client badged `BADGE_KEY_PASSPHRASE`;
  - start `blockstore`, `tls`, `ssh` and `wlan` only once `key-state` is `held` or `none`;
  - start the shell before them, so it can ask. Today `init`'s needs start the shell after `storefs`, and `storefs` after `blockstore`.
- **What it would not give:**
  - **offline guessing:** whoever copies the boot disk can try passphrases at Argon2id's cost alone, with no limit; a short passphrase falls;
  - **a changed boot image** that records the passphrase as it is typed: without a TPM nothing measures the boot, so nothing would notice;
  - **a forgotten passphrase** loses the device key. The store's media then open only with their recovery keys (308-STO-0028). The programs' secrets and the remembered networks, which open under the device key, are lost.

## Decisions (2026-11-04)

The kernel session agreed to the shape (`STO/2026-11-04T0844Z-KRN-…` on `internal-dev-messages`): a request to a prepared service before `held` gets `ERR_PEER`, `init`'s business; `key-state` answers at once in every state. The storage session decided the rest:

1. **Argon2id: 9216 KiB, 4 passes, 1 lane,** OWASP's row equivalent to 19 MiB and 2 passes, not the 19 MiB of the shape: a task's default memory quota is 16 MiB (`HEAP_MAX_BYTES`). Argon2's blocks come from `mind::mem::Pages`, freed after each derivation. The parameters are recorded in the file.
2. **A library crate, `passlock/`** (no_std, no libmind): `argon2` 0.5 (`default-features = false`, with `zeroize`) and `chacha20poly1305` 0.10, as `keystore` already uses. It owns the file and its outcomes (opened, wrong passphrase, damaged, another version by its number); its host tests run from `scripts/host_tests.sh`.
3. **A passphrase is never logged,** never on a command line, and wiped after use.
4. **Wrong tries:** from the third wrong one in a row, a try before the waiting time is refused at once with `budget`, nothing derived; the time doubles from 1 s up to 60 s; an opened one resets it. So no answer waits on a sleep. While one derivation runs, `keystore` answers nothing else, `key-state` included.
5. **A damaged `device.locked`:** `key-state` is `none`, the file is left as it is, nothing new is made, and the log says so. A sealed key is never replaced silently.
6. **Both files present** (a cut between writing `device.locked` and removing `device.key`): the sealed one wins (`waiting`); the plain one is removed after the next unlock.
7. **With a TPM:** as before; `key-state` says `held` (or `none`); `unlock` and `passphrase-set` are `invalid`.

Made while building it:

8. **The file, 97 bytes:** `MINDPWD1`; the version, one byte, 1; memory in KiB, passes and lanes, each a u32 little-endian; the salt (16) and the nonce (12); then the 32-byte seed sealed and its 16-byte tag. The 49 bytes before the seed are the associated data. The version is read before the length, so a file of another version is refused by its number whatever its length.
9. **Bounds on the parameters read:** at most 1 GiB, 32 passes and 16 lanes, and Argon2's own minimums. A file outside them is damaged, refused before any derivation, so a bit flipped in the passes cannot make an unlock run for hours. A later build may raise them.
10. **A new passphrase is written as `device.locked.new` first,** flushed, then the old file is removed and the new one renamed in its place. At boot, a missing `device.locked` with a whole `.new` uses the `.new` and renames it; a `.new` that is not whole, with no `device.locked`, replaced nothing and is ignored. So a cut leaves the old passphrase or the new one, never no key. The first passphrase is written the same way.
11. **A wrong `old`** in `passphrase-set` counts with `unlock`'s wrong tries, and waits the same.
12. **A passphrase outside 8 to 128 bytes** in `unlock` or as `old`: `denied` at once, nothing derived, not counted, since nothing was ever sealed under one. `new` outside them: `invalid`.
13. **`passphrase-set`'s other refusals:** while the key waits, `invalid`; without a device key, `no-key`; a non-empty `old` when none is set, or an empty one when one is, `denied`, nothing derived, not counted; a file not written, `no-key`, and the passphrase stays as it was.
14. **With a TPM and a `device.locked` on the disk:** the log says it is left as it is, and the TPM's flow runs as before.
15. **The badge is checked first:** a client without `BADGE_KEY_PASSPHRASE` is `denied` whatever the state, the TPM included.
16. **What is wiped:** the request's copy (the whole scratch buffer), the decoded strings in place, Argon2's blocks (by `passlock`, before the pages are freed), the derived key and every copy of the seed. **Not wiped:** temporaries a move leaves on the stack while the generated decoder builds the strings; what BLAKE2b keeps of the passphrase inside `argon2` (its `zeroize` feature wipes its own first hash and last block only); the client's request buffer, which libmind's generated client frees without wiping, and the kernel zeroes frames when it hands them out, not when they are freed. The last is the client's part (`APP`, or the IDL generator's).
17. **Each unlock and passphrase-set logs Argon2id's time** (`ARGON2ID … MS`), so the time is measured on every machine that runs one.
18. **`netcheck` tries the passphrase's calls with a fixed word of its own,** never one from its command line.
19. **`argon2` 0.5.3's AVX2 path:** on `target_os = "none"`, `cpufeatures` answers no, so the portable code runs on both architectures. rustc warns that the function enabling `avx2` on this soft-float target is to become an error (future-incompatible, rust-lang/rust#117938): a later toolchain needs `argon2` 0.6 or a patch.

## Acceptance criteria

- **Host tests:** the seal and the unseal, a wrong passphrase refused, and the parameters read back.
- **In QEMU without a TPM:** the key is held only after the passphrase. With 308-KRN-0205, the keyed services start after it.

## Progress (2026-11-04)

`keystore`'s side is built, by STO on its own branch; nothing of it is in `main` yet. The issue stays open for the criteria that need 308-KRN-0205 and `APP`.

**Built:**
- **`passlock/`,** a library crate: the file (decision 8), `seal`, `open`, `header`, the bounds on the parameters, and `Tries`, the count of wrong passphrases and the waits. Its host tests run in `scripts/host_tests.sh`.
- **`keystore.wit` 1.6:** `key-state` for any client, `unlock` and `passphrase-set` for `BADGE_KEY_PASSPHRASE` (6, `mind::network`) alone. The bindings are regenerated, and `KeyState` is in `tests/idl_fuzz_host.rs`'s list.
- **`keystore`** (`lock.rs`): without a TPM and with a sealed file, it starts serving with no key (`waiting`), and on an opened `unlock` it builds the certificate, the remembered networks and the rest, as at a normal start. `passphrase-set` writes `device.locked.new`, flushes it, puts it in place of the old file, flushes again, then removes `device.key` (decision 10). A damaged file, or one of another version, leaves no key and is left as it is (decision 5). Its log lines name every refusal, never a passphrase, and give Argon2id's time.
- **`netcheck key-state`, `unlock` and `passphrase-set`,** on its secrets client, with a fixed word of its own.

**Tested:**
- **Host:** `passlock`'s 12 tests pass, among them RFC 9106's own Argon2id vector (section 5.3, with its secret and associated data), the file rebuilt from its description byte for byte, and every byte of a file changed. Thirteen deliberate changes of its logic were each caught by a failing test: the header dropped from the associated data; the version not checked; Argon2i in place of Argon2id; a longer file taken; the memory not wiped; the parameters big-endian; a 7-byte passphrase taken; a changed magic called a wrong passphrase; no bound on passes (one test failed, and the every-byte test never ended: a flipped byte asked for 2^31 passes); the third wrong try free; the wait capped at 32 s; an opened passphrase not resetting the count; a try at the very end of the wait refused (`<=`). `scripts/host_tests.sh` passes whole.
- **QEMU, `tls` suite, x86 (`qemu64,+rdrand`, 4 CPUs) and aarch64 (`virt`, `-cpu max`), 8 PASS lines each, exit 0:**
  - `key-state` is `Held` on a boot without a TPM and on one with TPM A;
  - `unlock` and `passphrase-set` are `Denied` to netcheck's secrets client, which lacks badge 6, and logged (`NOT THE PASSPHRASE'S CLIENT`);
  - a `device.locked` of the file's form, placed from the host beside `device.key`, makes `keystore` wait: `key-state` `Waiting`, no key (`secret-get` `NoKey`), the unencrypted file kept;
  - the same file with its magic changed leaves no key: `key-state` `None`, nothing made in its place;
  - neither file is changed by those two boots.
- **A local experiment, not evidence of the committed build:**
  - **What was changed, never committed:** `keystore` built with the badge check bypassed, and `netcheck` with a second check sending a wrong word.
  - **Where:** x86 and aarch64 under TCG, 4 CPUs.
  - **What it showed:**
    - `passphrase-set` sealed the key, `device.key` was removed, and the next boot waited;
    - three wrong unlocks were `Denied`, the third with a wait of 1 s;
    - the fourth, at once, was `Budget`, nothing derived (`998 MS BEFORE THE NEXT TRY` on x86);
    - `key-state` answered `Waiting` at once;
    - two seconds later `unlock` opened the same key as before (same `MIND …` name), and a second `unlock` was `Invalid`.
  - **Argon2id's time, one derivation of 9216 KiB and 4 passes, as the log gives it:**
    - x86: 408 to 573 ms over the last two runs; 801 and 557 ms in the first;
    - aarch64: 540 to 730 ms over two runs.
- **These are tests of the configurations named, not a proof (MC-12.2).**

**Not done:**
- **`unlock` and `passphrase-set` by their real client.** No task holds badge 6 until `init` grants it to the shell (308-KRN-0205), and the shell has no prompt or command for it (`APP`). Until then a passphrase cannot be set or typed on the platform: every machine keeps the device key as before.
- **`init` waiting on `key-state`** (308-KRN-0205).
- **While a derivation runs,** about half a second to a second under TCG, `keystore` answers no request, `key-state` included.
- **The client's request buffer is not wiped** (decision 16): the client's part.
- **The parameters are measured under TCG only,** on no real machine.
- **A cut during the final flush of a change is not tested;** neither is a `.new` left whole by one.
- **With a TPM, a `device.locked` on the disk is logged and left as it is;** the TPM's flow runs as before (decision 14).
- **`argon2` 0.5.3's future-incompatibility warning** (decision 19).
- **What a passphrase does not give,** as in the shape: offline guessing on a copy of the disk at Argon2id's cost alone, a changed boot image recording the passphrase, and a forgotten passphrase losing the key.

## Related

[308-KRN-0205](308-KRN-0205-the-boot-waits-for-keystores-passphrase.md), [308-KRN-0204](308-KRN-0204-a-store-client-that-may-recover-the-volume-key.md), [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md).
