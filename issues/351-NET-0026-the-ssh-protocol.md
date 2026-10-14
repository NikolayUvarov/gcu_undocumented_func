# 351-NET-0026 — The SSH protocol as a host-tested library

**Type:** network (library) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) (phase 3), for [351-NET-0004](351-NET-0004-ssh-client.md) · **Roadmap:** track D · **Constitution:** MC-11.6, MC-11.9, MC-11.11, MC-12.2

The first part of [351-NET-0004](351-NET-0004-ssh-client.md), an SSH client that reads release files over SFTP. The service that runs it in the system, and the key service's purpose for its logins, are [351-NET-0027](351-NET-0027-the-ssh-service.md).

## Problem

No SSH code exists. The client must speak to an ordinary OpenSSH server, and it must not hold the device key: the key service signs for it (MC-11.9). QEMU has no SSH server of its own, so the protocol needs a test that does not depend on the system.

## Plan

A crate `ssh/`, `no_std` with `alloc`, with no libmind and no I/O of its own. The caller's `Io` moves the bytes, and its `Keys` gives the login key's public half and signatures.
- **The transport** (RFC 4253):
  - the version exchange;
  - packets of up to 35 000 bytes;
  - the key exchange: `curve25519-sha256` (RFC 8731), with OpenSSH's strict key exchange `kex-strict-c-v00@openssh.com` against CVE-2023-48795;
  - an `ssh-ed25519` host key (RFC 8709), accepted only when the SHA-256 of its blob is the caller's pin, and its signature of the exchange checked;
  - `chacha20-poly1305@openssh.com` both ways;
  - no second key exchange: a server that starts one ends the session.
- **Login** (RFC 4252): `publickey` with an `ssh-ed25519` key. The data it signs goes to the caller's `Keys`, and nothing is signed before the host key matched.
- **One session channel** (RFC 4254) with the `sftp` subsystem and the windows in both directions.
- **SFTP version 3:** open a file for reading, its size, reads of up to 32 KiB, close.
- **The licences** of the crates it uses go into THIRD_PARTY.md.
- **OpenSSH's server** for the tests goes into CI (`ci.yml`) and `scripts/setup_build_env.sh`, and `scripts/ci_local.sh` asks for it.

## Acceptance criteria

`ssh/tests/openssh.rs`, run by `scripts/host_tests.sh`, starts OpenSSH's server on 127.0.0.1 with its own host key and configuration, and:
- logs in with an ed25519 key, reads a file of 100 008 bytes whole in 32 KiB reads, a piece from its middle and nothing past its end, and closes it;
- gets SFTP's "no such file" for a missing file;
- refuses a server whose host key is not the pinned one, having signed nothing;
- is refused with a key the server does not know;
- ends the login when the signer gives no signature.

The crate builds for `x86_64-unknown-none` and `aarch64-unknown-none-softfloat`.

What this does not show: the client inside the system, through the network stack and the key service. That is 351-NET-0027.

## Related

[351-NET-0004](351-NET-0004-ssh-client.md), [351-NET-0005](../issues-done/351-NET-0005-persistent-device-key.done), [351-NET-0002](../issues-done/351-NET-0002-https-for-programs.done).
