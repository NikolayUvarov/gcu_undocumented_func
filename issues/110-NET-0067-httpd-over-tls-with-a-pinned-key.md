# 110-NET-0067 — `httpd` over TLS 1.3, with the key devices pin kept across renewals (stage 1, its second part)

**Type:** network (a host program, `tools/httpd/`) · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** open (made on `claude/110-NET-0067-httpd-over-tls`, 2026-11-11; landing left, after 110-NET-0066) · **Blocked by:** [110-NET-0066](110-NET-0066-httpd-serves-the-release-layout-over-http.md) (its base) · **Main task:** [110](110-our-own-web-server.md) · **Roadmap:** track D · **Constitution:** MC-9.6, MC-11.9, MC-12.3

## Problem

The updater fetches releases over HTTPS and pins the server's key (`update.txt`'s `pin`: the SHA-256 of the server's SubjectPublicKeyInfo). [110-NET-0066](110-NET-0066-httpd-serves-the-release-layout-over-http.md) serves the layout over plain HTTP only. [110](110-our-own-web-server.md) leaves the TLS library to be chosen and recorded.

## The choice recorded

- **rustls 0.23 over the RustCrypto crates,** as the `tls` service already uses them ([THIRD_PARTY.md](../THIRD_PARTY.md)): the same crates and versions, one more user. No C code and no other provider (no `ring`, no `aws-lc-rs`).
- **The cipher, hash, HMAC and key exchange glue** is `tls/src/provider.rs`'s, adapted from rustls-rustcrypto. The two copies are not shared, because `tls` is a `no_std` service with its own randomness and clock.
- **Our own server role** (stage 2's) would have served both sides. It is a TLS 1.3 server written from nothing; a host program does not need to wait for it.

## Plan

- **`--cert CHAIN.pem --key KEY.pem`:** TLS 1.3 alone (no 1.2), ALPN `http/1.1`.
  - Suites: AES-128-GCM, AES-256-GCM, ChaCha20-Poly1305. Groups: X25519, P-256. No client certificates.
- **The key:** ECDSA P-256 or Ed25519, in PKCS#8 or SEC1 PEM.
  - **RSA (PSS) only with `--allow-rsa`.** The `rsa` crate's private-key operations are not constant-time (RUSTSEC-2023-0071). They are for the tests' certificates, which are RSA.
  - **A key that is not the certificate's is refused at loading:** each key gives its SubjectPublicKeyInfo, and rustls compares it with the certificate's.
- **The pin:** the server prints `HTTPD: TLS 1.3, THE KEY'S SPKI SHA-256 <hex>`, the value `update.txt`'s `pin` takes, computed as `tls/src/pinned.rs` computes it.
- **Renewals** ([351-UPD-0031](351-UPD-0031-key-lifecycle-and-compromise-protocol.md)):
  - the certificate and key are read again when their files change, checked every 10 s, and new connections take the new pair; open ones keep theirs;
  - another key is said aloud, since a device pinning only the old one refuses the server;
  - a pair that does not load leaves the last one serving.
- **Refusals over TLS:** a connection over the limits is closed before its handshake. A failed handshake (TLS 1.2, plain HTTP) ends its connection alone and is logged.
- **The guide,** [docs/update/server.md](../docs/update/server.md) and [server_RU.md](../docs/update/server_RU.md): build, run, the root, TLS and the pin, renewals, limits, the log.

## Acceptance criteria

- Host tests (`tools/httpd/tests/tls.rs`, with `openssl` making the certificates and Python's `ssl` as the client):
  - each key kind serves a range over TLS 1.3: RSA, P-256 and Ed25519, and RSA in PKCS#1 and P-256 in SEC1;
  - TLS 1.2 and plain HTTP are refused, and logged;
  - the SPKI's hash is the one `openssl` gives for the certificate's public key;
  - an RSA key without `--allow-rsa`, and a key that is not the certificate's, are refused;
  - a certificate renewed over the same key is taken by new connections, and one that does not load leaves the last.
- THIRD_PARTY.md names `tools/httpd` beside `tls` for each crate, and the RSA note says where RUSTSEC-2023-0071 applies.

## Progress

**Made (2026-11-11)** on `claude/110-NET-0067-httpd-over-tls`, on 110-NET-0066's branch:
- `tools/httpd/src/tls.rs`, the server's flags, the guide and THIRD_PARTY.md;
- `cargo test`: 5 unit tests, 7 over plain HTTP, 4 over TLS, all passed; no warning;
- `curl` fetched a blob with `--pinnedpubkey` set from the printed SPKI and was refused with a wrong one.

## Related

[110](110-our-own-web-server.md), [110-NET-0066](110-NET-0066-httpd-serves-the-release-layout-over-http.md), [docs/update/updater.md](../docs/update/updater.md) (`pin`), [351-UPD-0031](351-UPD-0031-key-lifecycle-and-compromise-protocol.md).
