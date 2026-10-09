# 351-NET-0005 — A persistent device key

**Type:** network (key service) · **Owner:** `NET` track · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) (phase 3) · **Roadmap:** track D · **Constitution:** MC-11.9, Appendix B.6

Numbered from the request of `requests-NET.md` (the kernel track, for 351, recorded 2026-10-08 at the maintainer's request) by the storage session working the network track at the maintainer's request (2026-10-08).

## Problem

`keystore`'s Ed25519 key is made anew at every boot and kept only in memory, and it signs only TLS 1.3 CertificateVerify. A server cannot authorize a key that changes at every boot.

## Plan

- Keep the key across boots, sealed: encrypted with a key the device can recover and a copy of the disk alone cannot. TPM where present; otherwise state plainly what protects it.
- Add a signing purpose for the SSH login, under its own budget.
- Show the public key to the shell, so it can be put on the server.

## Acceptance criteria

The device key is the same after a reboot, its public key is shown, and it signs only the purposes listed.

## Related

[351](351-self-update.md), [351-NET-0004](351-NET-0004-ssh-client.md), issue 103.
