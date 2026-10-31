# 110 — Our own web server, `httpd`: releases first, on Linux and then on MIND Core

**Type:** network (main task: a server) · **Owner:** `NET` (the kernel session), with `UPD` for the release side · **Priority:** P1 · **Status:** open · **Blocked by:** — for stage 1; stage 2 by its own tasks: listening sockets and the listening grant ([110-NET-0052](110-NET-0052-listening-tcp-sockets.md)), the TLS server role · **Roadmap:** track C (update and distribution), track D (network) · **Constitution:** MC-5.1, MC-5.2, MC-9.4, MC-9.6, MC-10.3, MC-11.6, MC-11.9, MC-11.11, MC-12.1, MC-12.3

**Recorded by:** the kernel session, 2026-10-25, at the maintainer's request: "we must have our own web server, so that we can serve updates".

## Problem

- **What serves releases today:** "any static web server over the directory" ([docs/update/publishing.md](../docs/update/publishing.md), "Setting up a server").
  - `release.py publish` uploads `blobs/`, `releases/` and `channels/` over SSH.
  - A third-party server, such as nginx, serves them over HTTPS.
  - The tests use `scripts/serve_release.py`, a stand-in.
- **No server of the project's own** is written, configured and tested with the updater as one piece.
- **MIND Core serves nothing over the network:**
  - `netstack` has no listening socket for programs;
  - `tls` is a client only.

  So a MIND machine cannot give another machine its updates.

## What the server is trusted with, and what not

- **It is outside the TCB of an update's authenticity.** A release is signed with the boot key and the release key, and a device checks both (MC-9.6). A server that is broken into cannot forge a release.
- **What it can do is withhold or delay one.** The channel's expiry bounds such a freeze ([351-UPD-0025](../issues-done/351-UPD-0025-expiry-and-the-offline-policy.done)), and the floor in the boot records bounds a rollback (MC-9.4).
- **The profile says so** (MC-12.3): availability and freshness, not authenticity.

## Plan

### Stage 1: a host program for Linux, `tools/httpd/` (Rust, as `tools/mindlink` is)

- **What it serves:**
  - static files, read only, from one root: by default only the release layout (`blobs/<sha256>`, `releases/<version>/<arch>/…`, `channels/<name>`), with each name checked against its pattern;
  - no directory listings, no files outside the root, no symbolic links out of it, no names with a leading dot;
  - nothing that runs code.
- **HTTP:**
  - HTTP/1.1, `GET` and `HEAD`, with keep-alive;
  - one `Range` per request (206, 416), which the updater uses to resume (351-NET-0001);
  - `ETag` and `Last-Modified`;
  - `Cache-Control`: a blob is immutable, while a channel is fetched fresh each time.
- **Publishing stays as it is:** `release.py publish` writes into the root over SSH and renames the channel last. The server opens a file per request, so a reader sees the old channel or the new one, never a mix.
- **TLS 1.3:**
  - a certificate and key from files, reloaded on a signal without dropping connections;
  - **the key is kept across renewals,** since the updater pins the server's SPKI (`update.txt` `pin`). A new key is published as a second pin before it is used, as the key lifecycle says ([351-UPD-0031](351-UPD-0031-key-lifecycle-and-compromise-protocol.md)).
  - The TLS library is a choice to record: a third-party one (rustls) for the host program, with its licence in THIRD_PARTY.md, or our own server role (stage 2) on both sides.
- **Limits** (MC-5.1, 5.2), each refusal saying which:
  - the connections at once, and per client address;
  - the header's size (8 KiB);
  - a deadline for a request's head and between bytes (against slow clients);
  - requests per minute per client.
- **The log:** one line a request (time, client, path, status, bytes, duration), kept for a stated number of days (MC-10.3). It never holds a query string or a header's value.
- **A guide,** `docs/update/server.md` in English and Russian: install, the root, certificates and the pin, limits, logs.

### Stage 2: the same server as a MIND Core service

- **`netstack`:** listening TCP sockets for programs, with accept and a backlog, in `socket.wit`'s next minor version ([110-NET-0052](110-NET-0052-listening-tcp-sockets.md)).
- **`netpolicy`:** a listening grant (the program, the port, the networks it may be reached from), the inbound counterpart of a flow grant (MC-11.6; the same task).
- **`tls`:** a server role. Its private key lives in `keystore` and never leaves it (MC-11.9); the certificate chain comes from a file.
- **The content:**
  - a read-only VFS scope of the release directory;
  - later the block store, where 351-STO-0006 pins releases.
- **Parsing:** a request's head is parsed in a parser session that holds nothing else (MC-11.11).
- **What this gives:** a MIND machine serves its updates to the others, for instance on a network without the Internet.

### Stage 3: later (intention only)

- More than releases: a static site, the intake of [100003](100003-improve-the-system-from-the-phone.md)'s requests, a base for `msgd` (950).
- HTTP/2, then HTTP/3.
- Certificates through ACME.

## Acceptance criteria

- **Stage 1:**
  - host tests: the path rules (every refusal), `Range` parsing, the limits, a channel renamed during reads;
  - a fuzz target for the request parser (`ASR`'s harness);
  - the `update` and `linkupdate` checks in QEMU, x86 and aarch64, run against `httpd` instead of `scripts/serve_release.py`: release 6 fetched, a cut download resumed, an expired channel refused;
  - on the maintainer's server: a device updated from it, the configuration named (MC-12.1).
- **Stage 2:** a QEMU suite with two MIND machines on one network, where one serves a release and the other fetches it, boots it on trial and confirms it; a port outside the listening grant refused.
- **The profile** (Article 9) and docs/update say what the server is trusted with.

## Related

[351](351-self-update.md), [docs/update/publishing.md](../docs/update/publishing.md), [351-UPD-0031](351-UPD-0031-key-lifecycle-and-compromise-protocol.md), [351-STO-0006](351-STO-0006-releases-pinned-in-the-store.md), [000-NET-0047](../issues-done/000-NET-0047-http-for-api-clients.done) (the client side), [950](950-our-own-micro-messenger.md), [100003](100003-improve-the-system-from-the-phone.md).
