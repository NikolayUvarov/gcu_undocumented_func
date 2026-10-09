# 351-NET-0004 — An SSH client

**Type:** network (protocol) · **Owner:** `NET` track · **Priority:** P3 · **Status:** open · **Blocked by:** — ([351-NET-0005](../issues-done/351-NET-0005-persistent-device-key.done), a key the server can authorize, is done) · **Main task:** [351](351-self-update.md) (phase 3) · **Roadmap:** track D · **Constitution:** MC-11.6, MC-9.2

Numbered from the request of `requests-NET.md` (the kernel track, for 351, recorded 2026-10-08 at the maintainer's request) by the storage session working the network track at the maintainer's request (2026-10-08).

## Problem

No SSH code exists. The maintainer wants updates fetched over SSH as well as HTTPS. Most of the cryptography exists in `tls` and `keystore`: X25519, Ed25519, ChaCha20-Poly1305, AES-GCM, SHA-2 and HMAC.

## Plan

- SSH 2 client transport: `curve25519-sha256` key exchange, `ssh-ed25519` host keys checked against a pinned key, `chacha20-poly1305@openssh.com`.
- Public-key login with the device's key ([351-NET-0005](../issues-done/351-NET-0005-persistent-device-key.done)): a signing purpose `ssh-login` in `keystore`, under its own budget, for data of the form of an SSH `publickey` user authentication request (moved here from 351-NET-0005).
- The SFTP subsystem for reading files, or `exec cat` as a first step.
- The updater fetches the same release files over it. Authenticity still comes from the release signature, not from SSH.
- The licences of any crate used go into THIRD_PARTY.md.
- An SSH server on the device (for pushing) needs TCP listen in `netstack`. It is not part of this task.

## Acceptance criteria

In QEMU, against OpenSSH on the host, a service logs in with the device key, checks the pinned host key, and reads a file over SFTP into `vfs`.

## Related

[351](351-self-update.md), [351-NET-0001](../issues-done/351-NET-0001-http-downloads.done).
