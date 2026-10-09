# 351-NET-0002 — HTTPS for programs, and trust for the update server

**Type:** network (TLS) · **Owner:** `NET` track · **Priority:** P1 · **Status:** open · **Blocked by:** [requests-KRN.md](requests-KRN.md) ("A TLS client for programs": `REQUEST_TLS`), [requests-APP.md](requests-APP.md) (the shell lends its TLS client) · **Main task:** [351](351-self-update.md) · **Roadmap:** track D · **Constitution:** MC-11.6, MC-9.2, Appendix B.6

Numbered from the request "HTTPS downloads for a service" of `requests-NET.md` (the kernel track, for 351), by the storage session working the network track at the maintainer's request (2026-10-08).

## Problem

`download` (351-NET-0001) fetches over plain HTTP only. A program cannot hold a client of `tls`. The service trusts the roots in `tlsroots.pem` on the boot disk, which the build does not ship and nothing authenticates.

## Plan

- **`download` over `https://`:** a `Transport` over a `tls` session attached to its own flow grant, once a program can ask for the TLS client (`REQUEST_TLS`, kernel track; the shell lends it, tools track).
- **Trust for the update server, one of:**
  - its key pinned: a `connect` that checks the SHA-256 of the server's public key (SPKI) instead of a chain, in a new minor version of `idl/tls.wit`; the pin comes from the channel configuration;
  - roots shipped with the release and covered by its signature (the manifest lists them; 350).
- **Tests:** the release server of 351-UPD-0005 over HTTPS with the test CA, and pinned; a wrong pin and a wrong name refused.

## Acceptance criteria

`download` fetches 30 MiB over HTTPS from the release server with resume, verifying the server by a pinned key; a server with another key is refused.

## Related

[351-NET-0001](../issues-done/351-NET-0001-http-downloads.done), [351-UPD-0007](351-UPD-0007-updater-service.md), issue 103.
