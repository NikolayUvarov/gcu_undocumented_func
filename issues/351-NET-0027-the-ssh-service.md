# 351-NET-0027 — The SSH service, and logins signed by the key service

**Type:** network (service, interface) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P3 · **Status:** in progress (`download sftp://` gated on x86 and aarch64; one sub-criterion host-tested only) · **Blocked by:** the maintainer: whether `keystore`'s refusal of another form, host-tested only, closes it · **Main task:** [351](351-self-update.md) (phase 3), for [351-NET-0004](351-NET-0004-ssh-client.md) · **Roadmap:** track D · **Constitution:** MC-11.6, MC-11.9, MC-11.11, MC-12.4, Appendix B.6

The second part of [351-NET-0004](351-NET-0004-ssh-client.md): the protocol of [351-NET-0026](../issues-done/351-NET-0026-the-ssh-protocol.done) inside the system.

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

## Progress (2026-10-14)

Built, for x86_64 and aarch64:
- **What a login signs** is the `ssh` crate's `login` module: `signed_data` builds it for the client, and `fits` takes exactly that form with a given key and nothing else. Without its `client` feature the crate is `login` and `wire` alone, so the key service takes the check with no crypto of the client's.
- **`keystore`** (`idl/keystore.wit` 1.2):
  - `ssh-login` signs only data that `ssh::login::fits` takes with the device's own key;
  - only for `BADGE_KEY_SSH` (`mind::network`, 3), within 256 logins a boot, its own count beside the TLS signer's;
  - each signature and refusal is logged, as for TLS;
  - `public-key` gives the device's Ed25519 public key to anyone.
- **The service `ssh`** (`ssh_service/`, `idl/ssh.wit` 1.0):
  - `attach` takes a child of the client's flow;
  - `connect` logs in as the device, its key from `keystore` and each signature asked of it;
  - `open`, `size`, `read` (32 KiB) and `close-file`, then `close`;
  - sessions end with their process;
  - every session, refusal and pin mismatch is logged;
  - each request waits at most 15 s for the server, `connect` at most its timeout.
  - It is in the images (`02_build.sh`), and `init` does not start it yet.
- **Tests:**
  - `ssh/tests/login.rs`: a login with the key fits; another identifier length, message, user, service, method, a query without a signature, another algorithm or key, a byte more or less, and a TLS CertificateVerify do not.
  - `ssh/tests/openssh.rs` now checks that each login's data OpenSSH's server took is what `fits` takes.
  - The IDL fuzzer covers `ssh.wit`.
- **Left:**
  - `KRN`: `init` starts `ssh` with `keystore`'s client badged `BADGE_KEY_SSH` in slot 4, and `SLOT_SSH` and `REQUEST_SSH` for the shell and programs (requests-KRN.md);
  - `APP`: the shell lends it (requests-APP.md);
  - this track: `download`'s `sftp://` through `SLOT_SSH`, and the `net` suite's check against OpenSSH's server on the host.

## Progress (2026-10-20): gate 24

- **Gate 24** (`627aab32`, with `main` abe3d70, where 351-APP-0084 is) passed `sftp_runs`:
  - x86, "x86: boot, display, network…" on un-rizen;
  - aarch64, "aarch64: files, network and TLS" on un-rizen.
- **i7-930 has no OpenSSH server,** so its run of the x86 group skipped the check (`SKIP: download over SFTP needs OpenSSH's server …`) and passed the rest.
- **Met:**
  - the file read over SFTP with the device key, the host key matching its pin;
  - a wrong pin and a server that does not know the key, refused;
  - `keystore` logging each `ssh-login` signature.
- **Left of the criteria:** `keystore` refusing data of another form in QEMU. Only the SSH service holds the badge that asks for `ssh-login`, and it sends only `ssh::login::signed_data`, so no program in the system can send another form. The refusal is checked by the host tests (`ssh/tests/login.rs`, `fits`) and read in the code (`keystore/src/main.rs`). Whether that is enough to close is the maintainer's to say.

## Progress (2026-10-20): `download sftp://`

- **`init`** starts `ssh` with `keystore`'s client badged `BADGE_KEY_SSH`, and has `SLOT_SSH` and `REQUEST_SSH` (351-KRN-0115, in `main`).
- **`download`** (`c7c3a863`): `sftp://USER@HOST[:PORT]/PATH --pin HEX`. It asks for `REQUEST_SSH` and lends the service a child of its flow grant, then logs in with the device key. It reads in 32 KiB pieces from where the file ends; a cut is resumed by another attempt, and every call waits at most 60 s. `docs/network/downloads.md` 0.6 describes it.
- **The `net` suite's check** (`sftp_runs`): OpenSSH's server on the host's loopback, started for the test with its own host key (skipped without sshd):
  - a server without the device key: refused;
  - a wrong pin: refused (`HostKey`);
  - 300 008 bytes downloaded, matching their SHA-256, and `ALREADY COMPLETE` the second time;
  - `keystore` logs `SIGNED SshLogin` for the login.
- **Run locally** (x86, QEMU 8.2, TCG, 4 CPUs) on a merge with the tools branch, which has the shell lending the client: the whole `download` VM passed. Without that branch the shell lends no SSH client, so the gate waits for 351-APP-0084 in `main`.
- **Left:** the gate (x86 and aarch64); `keystore` refusing data of another form is checked by the host tests (`ssh/tests/login.rs`), as no program but the SSH service can ask it.

## Related

[351-NET-0004](351-NET-0004-ssh-client.md), [351-NET-0026](../issues-done/351-NET-0026-the-ssh-protocol.done), [351-NET-0002](../issues-done/351-NET-0002-https-for-programs.done) (the same design for TLS).
