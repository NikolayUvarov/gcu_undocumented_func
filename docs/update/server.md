# The release server, `httpd`

**Version:** 0.2 (2026-11-11) · **Track:** `NET` ([TRACKS.md](../../TRACKS.md)), main task [110](../../issues/110-our-own-web-server.md) · **Russian:** [server_RU.md](server_RU.md)

`tools/httpd` is the project's own web server for releases: a host program for Linux that serves `scripts/release.py`'s layout read-only. This is stage 1 of [110](../../issues/110-our-own-web-server.md). The `update` and `update_store` checks in QEMU run against it ([110-NET-0068](../../issues/110-NET-0068-the-update-checks-run-against-httpd.md)); no device has been updated from it on a real machine yet.

## What it is trusted with, and what not

- **Not with an update's authenticity.** A release is signed with the boot key and the release key, and a device checks both (MC-9.6). A server that is broken into cannot forge one.
- **It can withhold or delay a release.** The channel's expiry bounds that ([351-UPD-0025](../../issues-done/351-UPD-0025-expiry-and-the-offline-policy.done)).
- **It holds the TLS key that devices pin.** Whoever takes that key can stand in for the server, still without forging a release.

## Build and run

```
cd tools/httpd && cargo build --release
target/release/httpd /srv/mind --cert chain.pem --key key.pem --listen 0.0.0.0:443 --log /var/log/mind-httpd
```

- **The root** is the directory `release.py publish` writes: `blobs/`, `releases/`, `channels/`. Nothing else under it is served: each name is checked against its pattern, and links, dot names, queries and encoded bytes are refused.
- **Publishing stays as it is.** `publish` writes over SSH and renames the channel last. Each request opens its file once, so a reader gets the old channel or the new one whole.
- **Without `--cert` and `--key`** the server speaks plain HTTP, for a closed network or a test.

## TLS and the pin

- **TLS 1.3 only,** with AES-GCM and ChaCha20-Poly1305, X25519 and P-256: rustls 0.23 over the RustCrypto crates, as the `tls` service uses ([THIRD_PARTY.md](../../THIRD_PARTY.md)).
- **The key:** ECDSA P-256 or Ed25519, in PKCS#8 or SEC1 PEM. An RSA key loads only with `--allow-rsa`: the `rsa` crate's signing is not constant-time (RUSTSEC-2023-0071), so it is for the tests' certificates alone.
- **The pin.** At start the server prints `HTTPD: TLS 1.3, THE KEY'S SPKI SHA-256 <hex>`. That value is what `update.txt`'s `pin` takes ([updater.md](updater.md)).
- **Renewals keep the key.** The server reads the certificate and key again when their files change, checked every 10 s. Connections already open keep the old certificate.
  - A new certificate over the same key: `HTTPD: THE CERTIFICATE RELOADED, THE SAME KEY`.
  - A new key: the server says so, since a device that pins only the old key refuses it. A new key is published as a second pin before it is used ([351-UPD-0031](../../issues/351-UPD-0031-key-lifecycle-and-compromise-protocol.md)).
  - A pair that does not load, or a key that is not the certificate's: the last pair stays, and the server says why.

## Limits

Each refusal names the limit it met, in the response and in the log.

| Flag | Default | Refusal |
|---|---|---|
| `--connections` | 64 at once | 503 (over TLS, the connection is closed) |
| `--per-client` | 8 at once per address | 503 (as above) |
| `--per-minute` | 600 requests per address | 429, with `Retry-After` |
| (fixed) | a head of 8 KiB, 64 header lines | 431 |
| `--head-timeout` | 10 s for a request's head | 408 |
| `--idle-timeout` | 10 s between a head's bytes | 408; a kept-alive connection idle that long is closed |
| `--send-timeout` | 30 s for a write the client does not take | the connection is closed |

## The log

- One line a request: the time, the client's address, the method, the path, the status, the bytes sent, the duration, and a refusal's reason.
- One file a day, `httpd-YYYY-MM-DD.log`, in `--log`; files older than `--log-days` (30) are removed. Without `--log`, lines go to standard error.
- It never holds a query string or a header's value; bytes outside the names' alphabet show as `?`.

## The tests' hooks

The QEMU checks need a server that cuts a download midway, holds a response and records what was asked ([110-NET-0068](../../issues/110-NET-0068-the-update-checks-run-against-httpd.md)). Those hooks are built only with the Cargo feature `test-hooks`; a server built without it refuses `--test-hooks`.

- **`--test-hooks DIR`.** A path's name there is the path without its first `/`, each `/` as `+`.
  - `DIR/cut/<name>` holds a byte count: the next response for that path is cut after that many body bytes, and its connection closed.
  - `DIR/hold/<name>`: the next response for that path waits while the server's `DIR/held/<name>` is there, at most 10 s.
  - `DIR/requests`: a line a request, its path and its `Range` header (`-` without one).
- **`scripts/serve_httpd.py`** builds it so and runs it for the `update` and `update_store` suites. They set its deadlines to 60 s, since the emulated machines of a loaded host are slow.
- `scripts/serve_release.py` stays for the `net` suite's download checks, which need a malformed head no real server sends.

## Not done yet

- a device updated from the maintainer's server (110, stage 1);
- the same server as a MIND Core service (stage 2).
