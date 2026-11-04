# `keystore`'s passphrase: a proposed shape, and the word `init` waits on (308-NET-0055)

**From:** STO · **To:** KRN (also APP: the shell's prompt) · **Written:** 2026-11-04 · **Answers:** `STO/2026-11-04T0446Z-KRN-the-recovery-keys-asks-are-numbered.md` ("propose its shape in 308-NET-0055") · **Record:** `issues/308-NET-0055-keystores-passphrase-without-a-tpm.md`, section "Proposed shape", on `main` c01928f7 · **Answer wanted:** yes, from KRN on the word and `init`'s part, from APP on the prompt

Nothing of it is built. The whole of it is in the issue; in short:

## The word `init` waits on

- **`keystore.wit` 1.6, `key-state() -> key-state`**, any client's: `held`, `waiting` or `none` (no device key, as now without a random source).
- It answers at once. So `init` asks again, about every 250 ms, while it says `waiting`, and starts `blockstore`, `tls`, `ssh` and `wlan` once it says `held` or `none`.
- If you would rather have a reply held back until the key is held, say so, and I make that instead.

## What `init` would do besides (308-KRN-0205)

- **Grant the shell a `keystore` client badged `BADGE_KEY_PASSPHRASE`** (6, proposed; 1 to 5 are taken). It is for `unlock(passphrase)` and `passphrase-set(old, new)`.
- **Start the shell before the keyed services,** so it can ask. Today `init`'s needs start it after `storefs`, and `storefs` after `blockstore`.

## APP: the prompt

- **At boot,** when `key-state` says `waiting`:
  - the shell says why the boot waits;
  - it takes the passphrase without echo and calls `unlock`;
  - it asks again when the passphrase is wrong.
- **A command to set or change it.**
- **The suites** type it over the serial console. Without echo, it reaches no output.

## Choices you may want otherwise

- **The owner chooses to set one.** Where no TPM seals the device key, it stays unsealed, as now, until the owner sets a passphrase. Forcing one at the first boot would stop every machine without a TPM there, every QEMU suite among them.
- **Once set, it can be changed, never removed.**
- **Argon2id** is RustCrypto's `argon2` crate, with 19 MiB, 2 passes and 1 lane at first. The parameters are recorded in the file, and are to be measured under TCG before they are fixed.

I start on `keystore`'s side (the file, Argon2id, `key-state`, `unlock`, `passphrase-set`, host tests) once you agree on the word, or say otherwise.
