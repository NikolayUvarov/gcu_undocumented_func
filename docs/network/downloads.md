# Downloads: `mind::http` and `download`

**Version:** 0.3 (2026-10-09) · **Track:** `NET`, tasks [351-NET-0001](../../issues-done/351-NET-0001-http-downloads.done) and [351-NET-0002](../../issues-done/351-NET-0002-https-for-programs.done) (HTTPS) for main task [351](../../issues/351-self-update.md), and [109-NET-0009](../../issues-done/109-NET-0009-download-through-the-parser.done) (the head parsed in `parse`) · **Roadmap:** track D · **Constitution:** MC-11.6, MC-3.11, Appendix B.6

A program fetches a file over HTTP/1.1 with the network its launcher lent it and writes it into a file it may write. A download cut midway, or one a program gave up on, goes on from where it stopped instead of starting again. This page describes what is built. The updater (351-UPD-0007) will use the same library for releases.

## `mind::http`

`libmind/src/http.rs` is `no_std`, without allocation, and knows nothing of sockets or files. It sends one GET and streams the body somewhere:

- **`Url::parse`** takes `http://` and `https://` URLs: the authority as the `Host` header names it, the host, the port (80, 443 or the one given) and the path. Spaces and control characters are refused.
- **`get(transport, url, offset, sink, parser)`** sends `GET <path> HTTP/1.1` with `Host`, `Accept-Encoding: identity` and `Connection: close`. From an offset other than 0 it adds `Range: bytes=<offset>-`. It reads the head, at most 8 KiB, up to its blank line, and has the `Parser` turn it into a typed `Head`: the status, `Content-Length`, `Content-Range`, whether the body is chunked. In the system the parser is the parser service `parse` (`mind::parse::Service`, 109-NET-0008); in the host tests it is `Local`, the same code (`parse_head`) in the process. `get` then checks the typed head itself, whoever parsed it (MC-11.5):
  - **200:** the whole body. `Content-Length` is required. A server that ignores the range sends the file again from 0, and the sink is told so.
  - **206:** the body from `offset`. `Content-Range` must start exactly there, and a `Content-Length` must agree with it.
  - **416** with `Content-Range: bytes */<offset>`: nothing is left past `offset`, so the file is complete.
  - **Refused:** any other status, redirects included (`Status(code)`); a chunked body (`Chunked`); a body without a length (`Length`); a malformed head (`Head`); a parser that could not be asked (`Parser`).
- **The result is `Got { start, end, total }`.** A connection that closes or fails after the body began is not an error. `end` says how far the body got, and the caller asks again from there. Bytes past the length the head announced are ignored.
- **`Transport`** (send all, receive at least one byte or 0 at the end) and **`Sink`** (`begin` at an offset, `write` at an offset) are the caller's. So the network policy that applies to the caller applies to its downloads, and the library holds no authority of its own.

Host tests (`tests/http_host.rs`) cover:
- URLs and the request sent;
- bodies taken in pieces of 1 byte to 9 000;
- a cut resumed with `Range`, and a server that sends the whole file again;
- a complete file (416);
- each refusal;
- a sink that refuses;
- the head parsed on its own, and `get`'s checks against a parser that lies about the range or the length, or cannot be asked.

## `download`

```
download FILE URL [--sha256 HEX] [--tries N] [--pin HEX]
```

- **What it asks its launcher for** (MC-3.11: the request grants nothing):
  - `REQUEST_NETWORK`: a flow grant from the policy broker. It reaches only what `netpolicy.txt` names for `download`, for the term and up to the volume given there (MC-11.6). A line `download dns` lets it look names up.
  - `REQUEST_FILE`: a client confined to the directory of `FILE`. It is writable on `ram:` and in `data/`, and read-only elsewhere.
  - `REQUEST_PARSE`: a client of the parser service (109-NET-0009). Without it `download` refuses (`NO PARSER SERVICE`) rather than parse a head in the process that holds the grant and the file.
  - `REQUEST_TLS`: the launcher's client of the TLS service, for `https://` (351-NET-0002). The shell lends it only with a flow grant, and to a script's program only if the script declares `tls` (351-APP-0017). Without it `download` refuses an `https://` URL (`HTTPS NEEDS A TLS CLIENT, AND NONE WAS LENT`).
  - `REQUEST_CONSOLE`.
- **What it does:**
  - It opens `FILE` without emptying it. If the file already has bytes, it asks from there (`DOWNLOAD: RESUMING`).
  - After a connection cut midway (`DOWNLOAD: CONNECTION CUT AT n OF total, RESUMING`), or a connection that did not answer, it connects again, up to `--tries` connections in all (5 by default).
  - It writes the body in pieces of 64 KiB, and gives up with the bytes it has (`GAVE UP AFTER n CONNECTIONS AT m BYTES`). The next run goes on from there.
  - It stops at once on what asking again would not change: a status, a malformed answer, a file it cannot write.
- **At the end** it reads the file back and prints its SHA-256. With `--sha256` it says whether that matches, and exits with status 1 if not.
- **The grant's term and volume must cover the download.** When the term runs out, the stack closes the connection and the broker takes the grant back (`DOWNLOAD: NO NETWORK GRANT`). The next run gets a new grant and goes on from where the file ends.
- **A policy line**, for example:

  ```
  download 10.0.2.2 tcp 8443 3600 67108864   # the release server: an hour, up to 64 MiB
  ```

  The address may be a host name (`download updates.example.org tcp 443`). The broker looks it up when it makes the grant, at the file's `resolver` line or else the stack's DNS server, and the grant keeps that address (351-NET-0003).

Plain HTTP gives neither confidentiality nor authenticity. For a release, authenticity comes from its signature, which the updater checks (351-UPD-0005, 0007), not from the transport.

## HTTPS

For an `https://` URL `download` runs each connection as a session of the TLS service (351-NET-0002):

- **The session runs on `download`'s own flow grant.** `attach` lends the TLS service a child of the grant, so the policy, its term and its volume apply to the encrypted connection as to a plain one. The TLS service has no network access of its own.
- **The server is verified by the roots or by a pinned key:**
  - **Without `--pin`:** the certificate chain must lead to a root in `tlsroots.pem` on the boot disk and be valid for the URL's host, as for the shell's `https`.
  - **With `--pin HEX`:** the SHA-256 of the server certificate's SubjectPublicKeyInfo must be `HEX` (`connect-pinned`, `idl/tls.wit` 1.1). No root, name or validity period is checked, but the server must still sign the handshake with that key. A pinned key is trusted whoever signed its certificate, so an update server needs no CA: the pin can come with the channel's configuration.

    The pin of a certificate is `openssl x509 -in cert.pem -pubkey -noout | openssl pkey -pubin -outform der | sha256sum`.
- **A refusal is final.** `TLS: Certificate` (an untrusted server or another key, logged by `tls`: `REFUSED: CERTIFICATE …` or `REFUSED: NOT THE PINNED KEY`) is not retried. Neither is a port the grant does not name (`TLS: Denied`). A connection refused or timed out is retried like a plain one.
- **A cut is resumed with `Range` as over HTTP.** A connection closed without TLS's close_notify does not complete a file: the body is complete only at the length its head gave.
- **TLS takes random bytes from RDRAND (RNDR on aarch64) only.** Without them the TLS service refuses every connection.

## Tested

These tests run in the `net` suite, on x86 and aarch64, in QEMU with user networking. They boot their own VM, and on x86 its boot disk is on AHCI: the IDE driver's port I/O, emulated, takes minutes for 30 MiB. 30 MiB takes about 90 s on x86 under TCG, almost all of it in writing the file. This is not a measure of throughput. On aarch64 the file is 8 MiB, cut at 3 MiB: there 30 MiB took more than 10 minutes. The server is `scripts/serve_release.py`, which now answers single byte ranges, over plain HTTP. It is given a hook that cuts the first response for a path.
- 30 MiB (8 MiB on aarch64) through `download`'s own grant: the first response is cut at 10 MiB (3 MiB), and the rest is asked for with `Range` from there.
- The SHA-256 is checked in the system. A second run finds the file complete (416) and changes nothing.
- A run given up on after one connection, at 50 000 of 200 000 bytes, is resumed by the next run.
- These are refused:
  - a missing file (`Status(404)`);
  - a port the grant does not name (`Denied` from the stack, over http and https);
  - a file outside the writable directories.
- **Over HTTPS** (351-NET-0002), from the same release server with the TLS suite's test certificates; the VM's processor has RDRAND:
  - the same 30 MiB (8 MiB) through a TLS session on `download`'s own grant, cut and resumed with `Range`, the server verified by its pinned key;
  - a file verified by the roots;
  - refused: a wrong pin (`NOT THE PINNED KEY`) and a server from a CA nobody trusts (`UnknownIssuer`);
  - that server's own key pinned is accepted;
  - `--pin` with an `http://` URL is refused;
  - from a script that does not declare `tls`, `download` gets no TLS client and refuses `https://`.
- The heads of all these were parsed in `parse` (109-NET-0009). A malformed head from the server is refused there (`DOWNLOAD: HTTP: Head`) and logged by `parse` with `download`'s PID. `parse` holds two capabilities, its endpoint and a `logd` client (`stat caps`). Started from a script that grants it everything but `parse`, `download` refuses. Killed, `parse` is restarted by `init` behind the same endpoint, and the next download goes through it.

## Not provided yet

- **Roots shipped with a release** and covered by its signature: the roots are whatever `tlsroots.pem` holds, which nothing authenticates; a pin is the way to trust the update server for now.
- **HTTPS by name with a pin from the channel:** the updater's (351-UPD-0007).
- **Chunked bodies, redirects, keep-alive, several connections at once, IPv6.**
- **Fast writes of a large file.** `vfs_server` writes a file one 512-byte sector per block request, and walks the file's cluster chain from its start on every write. So writing slows down as the file grows. That is `vfs_server`'s, not this task's.
- **A parser instance per session.** One `parse` process serves every program, one request at a time, and keeps nothing between requests; a fresh process per session needs a kernel and loader change ([109](../../issues-done/109-session-parsers.done)).
