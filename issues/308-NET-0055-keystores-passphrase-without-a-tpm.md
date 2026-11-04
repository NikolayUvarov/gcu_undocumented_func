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

**Proposed by** STO, which makes this task. `init`'s part (308-KRN-0205) asked for it: "the word I need from `keystore`". Nothing of it is built. It is a proposal, not a guarantee (MC-12.3).

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

## Acceptance criteria

- **Host tests:** the seal and the unseal, a wrong passphrase refused, and the parameters read back.
- **In QEMU without a TPM:** the key is held only after the passphrase. With 308-KRN-0205, the keyed services start after it.

## Related

[308-KRN-0205](308-KRN-0205-the-boot-waits-for-keystores-passphrase.md), [308-KRN-0204](308-KRN-0204-a-store-client-that-may-recover-the-volume-key.md), [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md).
