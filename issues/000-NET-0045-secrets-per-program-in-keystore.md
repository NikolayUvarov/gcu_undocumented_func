# 000-NET-0045 — Secrets per program in `keystore`: a token, a password or an API key that only its program reads

**Type:** network (`keystore`, `idl/keystore.wit`) · **Owner:** `NET` · **Priority:** P1 · **Status:** open · **Blocked by:** — (the program's identity: [000-KRN-0170](../issues-done/000-KRN-0170-a-programs-own-data-directory.done)'s rule, given by [000-KRN-0182](000-KRN-0182-a-programs-secrets-client.md)) · **Main task:** — (for 253, 254, 260, 261) · **Constitution:** MC-3.1, MC-10.3, MC-11.9, MC-12.4

Recorded by the network track on 2026-10-25, for the plan [docs/communications-and-agents.md](../docs/communications-and-agents.md). 253, 254, 260 and 261 are each blocked by "where the token lives".

## Problem

- **The clients hold secrets:**
  - `tgtxt` a bot token, then a Telegram authorization key;
  - `mail` a password or an OAuth token;
  - `agent` and `ask` a provider's API key.
- **A secret must be readable by its program alone.** It must never appear in a log, a command line or a session file (MC-10.3, MC-11.9).
- **`keystore` keeps the device key and the remembered Wi-Fi networks** in its private directory, for named services. It has nothing for an application.

## Plan

- **`keystore.wit`, its next minor version:**
  - `secret-put(name, value)`, `secret-get(name)`, `secret-delete(name)`, `secret-list()`;
  - for the caller's program, known by the identity the loader set on its client (000-KRN-0170), never by a name the caller gives;
  - a name is short ASCII, and a value is at most a stated size (16 KiB, enough for an OAuth refresh token or an MTProto key).
- **Storage:** in `keystore`'s private directory, one file per program, written whole. Sealed by a TPM where there is one (351-NET-0006, when it lands). Without a TPM the limit is stated: whoever has the disk in hand can read them.
- **Putting one in:** a shell command (`secret set <program> <name>`, a request to `APP`, or a small tool of this track's) reads the value from the console without echo, never from the command line. `secret list <program>` shows names only.
- **The log:** names `secret-get` and `secret-put` by program and name, never the value.

## Progress (2026-10-28)

**Made, for the program's own side:**

- **`idl/keystore.wit` 1.4.0:**
  - for the loader's badge (`BADGE_KEY_LOADER`) alone: `secrets-client`, `secrets-name` and `secrets-adopt`;
  - for a secrets client: `secret-put`, `secret-get`, `secret-delete` and `secret-list`;
  - the error `missing`.
- **How a client knows its program.**
  - The loader asks for a client for each program it runs by its name ([000-KRN-0182](000-KRN-0182-a-programs-secrets-client.md), `SLOT_SECRETS`, ABI 24), names it for the task, and names the task after the spawn.
  - `keystore` answers a client only for that task, or for the first that calls on it before the loader names one, and only about that program's secrets.
  - A client ends with its task, or after a minute when no task took it up, and is revoked then.
  - The client is minted by `keystore` from its own endpoint, with a badge from 0x100 up.
- **Storage:** `system/keystore/secrets/<program>.kept` (`keystore/src/secrets.rs`):
  - the file is `MINDSEC1`, a random 12-byte nonce, then the record sealed with ChaCha20-Poly1305 and its tag;
  - the key is the SHA-256 of a label, the device key's seed and the program's name, and the authenticated data names the program too, so a file copied under another program's name does not open;
  - a write goes to `<program>.new`, is flushed, and then replaces the kept file. A read takes `.new` only when the kept one is gone.
  - Without a TPM, the device key is on the same disk, so whoever has the disk has the secrets: stated in the profile.
- **The record** (`keystore/src/secret_file.rs`):
  - names are 1 to 32 letters, digits, `.`, `_` or `-`;
  - values are 1 to 16384 bytes;
  - at most 16 secrets and 65536 bytes of names and values for one program.
- **The log** names the program, the secret and the PID, never a value. `keystore` wipes its copies of a value: the request buffer after a `secret-put`, the decrypted record and the reply's copy after a `secret-get`.
- **A test program.** `netcheck`, a console program of this track, asks for `REQUEST_SECRETS`:
  - `secret-put:NAME:FILE` keeps a file's bytes as a secret;
  - `secret-get:NAME` prints the length and the SHA-256, never the value;
  - `secret-delete:NAME` and `secrets`.
- **Host tests** `tests/secrets_host.rs` (in `scripts/host_tests.sh`): the record read back, names and values refused, the limits, records the decoder refuses.

**How a secret is put in, changed from the plan.** Each program keeps its own: it reads the value from its console without echo, or from a file, as `netcheck` does. That way the value is never on a command line or in a shell history.
- The owner's tool `secret set <program> <name>` would need a client that writes for a program named by its caller.
- No such client exists: no badge may name another program's secrets.
- Recorded here as left; it would need a shell part from `APP`.

## Acceptance criteria

- **Host tests:** the file format, a refusal of a too-long name or value.
- **In QEMU, x86 and aarch64:**
  - a test program puts a secret and reads it back after a reboot;
  - another program cannot read it, even by asking for the same name;
  - the value appears in no log line and no shell history.
- **Docs:** `idl/keystore.wit` with its new version; the profile's Article 11 row says where secrets live and the limit without a TPM.

## Related

[000-KRN-0170](../issues-done/000-KRN-0170-a-programs-own-data-directory.done), [351-NET-0006](351-NET-0006-device-key-sealed-by-a-tpm.md), [253](253-telegram-and-whatsapp-text-client.md), [260](260-ai-agents-on-the-platform.md), [261](261-mail-client.md).
