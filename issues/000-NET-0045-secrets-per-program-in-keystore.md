# 000-NET-0045 — Secrets per program in `keystore`: a token, a password or an API key that only its program reads

**Type:** network (`keystore`, `idl/keystore.wit`) · **Owner:** `NET` · **Priority:** P1 · **Status:** open · **Blocked by:** [000-KRN-0170](../issues-done/000-KRN-0170-a-programs-own-data-directory.done) (the program's identity) · **Main task:** — (for 253, 254, 260, 261) · **Constitution:** MC-3.1, MC-10.3, MC-11.9, MC-12.4

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

## Acceptance criteria

- **Host tests:** the file format, a refusal of a too-long name or value.
- **In QEMU, x86 and aarch64:**
  - a test program puts a secret and reads it back after a reboot;
  - another program cannot read it, even by asking for the same name;
  - the value appears in no log line and no shell history.
- **Docs:** `idl/keystore.wit` with its new version; the profile's Article 11 row says where secrets live and the limit without a TPM.

## Related

[000-KRN-0170](../issues-done/000-KRN-0170-a-programs-own-data-directory.done), [351-NET-0006](351-NET-0006-device-key-sealed-by-a-tpm.md), [253](253-telegram-and-whatsapp-text-client.md), [260](260-ai-agents-on-the-platform.md), [261](261-mail-client.md).
