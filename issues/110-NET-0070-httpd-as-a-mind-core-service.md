# 110-NET-0070 — `httpd` as a MIND Core service: the release layout over HTTPS from a read-only scope

**Type:** network (a new service `httpd/`, `parse`, `libmind` `http_serve`, the grants in `init` as a `KRN` part) · **Owner:** `NET` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [110](110-our-own-web-server.md) (stage 2) · **Roadmap:** track C, track D · **Constitution:** MC-5.1, MC-5.2, MC-9.4, MC-10.3, MC-11.6, MC-11.9, MC-11.11, MC-12.1, MC-12.3

## Problem

Stage 1 of [110](110-our-own-web-server.md) serves releases from a Linux machine (`tools/httpd`). Stage 2 has a MIND machine serve them to the others, for instance on a network without the Internet.

Most of what it needs exists:
- listening sockets and the listening grant ([110-NET-0052](../issues-done/110-NET-0052-listening-tcp-sockets.done));
- the TLS server role, with the device key kept in `keystore` ([110-NET-0069](110-NET-0069-the-tls-services-server-role.md)).

What is missing is the server itself, and a parser of request heads outside it (MC-11.11).

## Plan

1. **Request heads parsed by `parse`.**
   - `parse.wit` 2.1 gets `http-request`, with the rules of `tools/httpd`'s `head.rs`:
     - GET and HEAD only;
     - HTTP/1.1 or 1.0, and `Host` for 1.1;
     - no folded line;
     - at most 64 header lines;
     - a target of at most 1024 bytes.
   - It answers with the status a refused head gets (400, 405, 431 or 505). For a head that is served, it gives:
     - the method;
     - the target;
     - whether the connection stays open after the response;
     - `Range` and `If-None-Match`;
     - whether the request has a body.
   - The rules are in `libmind` `http` (`parse_request`), with host tests that hold the cases of `tools/httpd`'s.
2. **The service `httpd/`** (`mind::release`'s layout, as `tools/httpd` serves it).
   - **Content:** `blobs/<sha256>`, `releases/<version>/<arch>/MANIFEST` and `MANIFEST.SIG`, `channels/<name>`. They are read from a read-only scope of the owner's `data/releases/`, which `init` lends. The scope is not made: the service waits for the owner's directory.
   - **Responses:** `Range` (one range), `ETag` and `If-None-Match`, and `Cache-Control` as `tools/httpd` sends them.
   - **TLS:** it listens through `tls` on TCP 443, which the policy's listening grant for program `httpd` must name. There is no plain HTTP.
   - **Limits, each refusal saying which:**
     - connections at once (7, what one client's `tls` sessions and sockets allow beside the listener);
     - the head's size (8 KiB);
     - deadlines for the head and between bytes (10 s);
     - requests per minute per client address.
   - **The log:** one line a request in the system log (the client, the method, the path, the status, the bytes, the time). It never holds a query string or a header's value.
3. **`init` (a `KRN` part).**
   - It starts `httpd` only when the policy gives program `httpd` a flow, as it starts `timed`.
   - Its grants: that flow; a TLS client with the right to the device certificate (unbadged); the parser; the clock; the read-only scope.
   - Nothing else: no other files, no keys, no devices.

## Acceptance criteria

- **Host:** `libmind`'s request-head rules, with every case of `tools/httpd`'s head tests; the layout and range rules, with every case of `tools/httpd`'s.
- **QEMU, x86 and aarch64**, the policy holding `listen httpd tcp 443 from 10.0.2.0/24`, and a release layout in `data/releases/`:
  - a host client (Python's `ssl` and `http.client`, TLS 1.3, pinning the device's SPKI) fetches a channel and a blob whole;
  - it resumes the blob with `Range` (206), and gets 304 for its `ETag`;
  - it gets 404 for a path outside the layout, 400 for a query string, and 405 for POST;
  - each request is one log line;
  - 0069's criteria through it: `SIGNED TlsServer`, a handshake not finished cut after 10 s, a `tlsserver.pem` for another key refused.
- **Without the grant:** `httpd` is not started, and `init` says why.
- **The profile** (Article 9) and `docs/update/server.md` say what the MIND service is trusted with and what it is not.

Not in this task: the two-machine suite of stage 2's criterion (one MIND machine serves, the other updates from it), which needs the updater's server address set to a MIND machine (a later task).

## Progress

**Made (2026-11-12)** on `claude/110-NET-0069-tls-server-role`, with 110-NET-0069:
- **`libmind/src/http_serve.rs`:** `tools/httpd`'s rules for a server, without allocation: `parse_request`, `asked`, `matches_tag`, `resolve`.
  - `tests/http_serve_host.rs` holds every case of `tools/httpd`'s head, range and layout tests, and what the parser's record keeps (its bounds, the first of each header, a body). It passes.
  - **One difference:** a target over 1024 bytes is refused with 414, where `tools/httpd` reads it and answers 404.
- **`parse.wit` 2.1 `http-request`,** served by `parse` (`mind::parse::to_request`, `from_request`). `tests/idl_fuzz_host.rs` lists its types and `tls.wit` 1.3's `Accepted`.
- **`httpd/`:** one loop over its connections through `tls`, no call waiting for a peer; each head sent to `parse`; each file read from `init`'s read-only scope of `data/releases/`; the limits and the log as planned.
- **`init`** (the `KRN` part):
  - `httpd` after `tls` in the boot images, **handoff 33** (`common/abi.rs`, `tests/boot_abi_host.rs`);
  - its grants; not started without the policy's flow (`THE POLICY GIVES IT NO LISTENING GRANT`);
  - `NEEDS` on `netpolicy`, `tls` and `parse`.
- **`webcheck listen:PORT`,** for the refusal of a program's client.
- **`tests/qemu_smoke_probe.py` removed:** a probe of 000-KRN-0233, committed to `main` by mistake in the merge 1a18628c.
- **Tested** (this container, x86, the `tls` suite; see 110-NET-0069): every request of the criteria and their log lines. Without the grant, `init` says `httpd NOT STARTED: THE POLICY GIVES IT NO LISTENING GRANT`. The host tests the change touches pass.
- **Docs:**
  - `docs/update/server.md` and `server_RU.md` 0.3 (the MIND service);
  - the profile's TCB and bootstrap's grants;
  - the IDL table and `docs/api`.
- **Left:** aarch64; the gate.

## Related

- [110](110-our-own-web-server.md): stage 2.
- [110-NET-0069](110-NET-0069-the-tls-services-server-role.md): the TLS server role.
- [110-NET-0066](../issues-done/110-NET-0066-httpd-serves-the-release-layout-over-http.done): stage 1's server and its rules.
