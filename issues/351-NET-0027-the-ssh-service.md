# 351-NET-0027 — The SSH service, and logins signed by the key service

**Type:** network (service, interface) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P3 · **Status:** open · **Blocked by:** [351-NET-0026](351-NET-0026-the-ssh-protocol.md) (the protocol); `KRN` for starting the service and lending its clients ([requests-KRN.md](requests-KRN.md), "The SSH service in the boot services") · **Main task:** [351](351-self-update.md) (phase 3), for [351-NET-0004](351-NET-0004-ssh-client.md) · **Roadmap:** track D · **Constitution:** MC-11.6, MC-11.9, MC-11.11, MC-12.4, Appendix B.6

The second part of [351-NET-0004](351-NET-0004-ssh-client.md): the protocol of [351-NET-0026](351-NET-0026-the-ssh-protocol.md) inside the system.

## Problem

A program that reads release files over SFTP must neither hold the device key nor parse the server's bytes next to it (MC-11.9, MC-11.11). The TLS service already works this way for HTTPS (351-NET-0002), and the SSH client follows it.

## Plan

- **`keystore`** (`idl/keystore.wit` 1.2):
  - a signing purpose `ssh-login` for its own badge (`BADGE_KEY_SSH`), under its own budget;
  - only for data of the form an SSH `publickey` login signs: a 32-byte session identifier, `ssh-connection`, `publickey`, `ssh-ed25519` and the device key's own public key.
- **The service `ssh`** (`idl/ssh.wit` 1.0), as `tls` does it:
  - a program lends it its flow grant (`attach`);
  - `connect(host, port, user, host-key pin, timeout)` logs in as the device;
  - `open`, `size`, `read`, `close` for files;
  - every session, refusal and pin mismatch is logged;
  - it holds its endpoint, `keystore`'s client with `BADGE_KEY_SSH`, and the log: no files, no network of its own.
- **`download`** takes `sftp://user@host[:port]/path` with `--hostkey SHA256:...`, and writes the file as it does for HTTP.
- **`KRN`'s part:** `init` starts `ssh` and lends it `keystore`'s client, and a slot and request flag for programs. The shell's lending is the tools track's. Both are asked in the request files.

## Acceptance criteria

In QEMU, the `net` suite on x86 and aarch64, against OpenSSH's server on the host:
- `download` reads a file over SFTP into `data/`, logged in with the device key, the host key matching its pin;
- a wrong pin and a key the server does not know are refused;
- `keystore` logs each `ssh-login` signature and refuses data of another form.

## Related

[351-NET-0004](351-NET-0004-ssh-client.md), [351-NET-0026](351-NET-0026-the-ssh-protocol.md), [351-NET-0002](../issues-done/351-NET-0002-https-for-programs.done) (the same design for TLS).
