# Downloads: `mind::http` and `download`

**Version:** 0.1 (2026-10-08) · **Track:** `NET`, task [351-NET-0001](../../issues-done/351-NET-0001-http-downloads.done), for main task [351](../../issues/351-self-update.md) · **Roadmap:** track D · **Constitution:** MC-11.6, MC-3.11, Appendix B.6

A program fetches a file over HTTP/1.1 with the network its launcher lent it and writes it into a file it may write. A download cut midway, or one a program gave up on, goes on from where it stopped instead of starting again. This page describes what is built. The updater (351-UPD-0007) will use the same library for releases.

## `mind::http`

`libmind/src/http.rs` is `no_std`, without allocation, and knows nothing of sockets or files. It sends one GET and streams the body somewhere:

- **`Url::parse`** takes `http://` and `https://` URLs: the authority as the `Host` header names it, the host, the port (80, 443 or the one given) and the path. Spaces and control characters are refused.
- **`get(transport, url, offset, sink)`** sends `GET <path> HTTP/1.1` with `Host`, `Accept-Encoding: identity` and `Connection: close`. From an offset other than 0 it adds `Range: bytes=<offset>-`. It reads the head, at most 8 KiB, and then:
  - **200:** the whole body. `Content-Length` is required. A server that ignores the range sends the file again from 0, and the sink is told so.
  - **206:** the body from `offset`. `Content-Range` must start exactly there, and a `Content-Length` must agree with it.
  - **416** with `Content-Range: bytes */<offset>`: nothing is left past `offset`, so the file is complete.
  - **Refused:** any other status, redirects included (`Status(code)`); a chunked body (`Chunked`); a body without a length (`Length`); a malformed head (`Head`).
- **The result is `Got { start, end, total }`.** A connection that closes or fails after the body began is not an error. `end` says how far the body got, and the caller asks again from there. Bytes past the length the head announced are ignored.
- **`Transport`** (send all, receive at least one byte or 0 at the end) and **`Sink`** (`begin` at an offset, `write` at an offset) are the caller's. So the network policy that applies to the caller applies to its downloads, and the library holds no authority of its own.

Host tests (`tests/http_host.rs`) cover:
- URLs and the request sent;
- bodies taken in pieces of 1 byte to 9 000;
- a cut resumed with `Range`, and a server that sends the whole file again;
- a complete file (416);
- each refusal;
- a sink that refuses.

## `download`

```
download FILE URL [--sha256 HEX] [--tries N]
```

- **What it asks its launcher for** (MC-3.11: the request grants nothing):
  - `REQUEST_NETWORK`: a flow grant from the policy broker. It reaches only what `netpolicy.txt` names for `download`, for the term and up to the volume given there (MC-11.6). A line `download dns` lets it look names up.
  - `REQUEST_FILE`: a client confined to the directory of `FILE`. It is writable on `ram:` and in `data/`, and read-only elsewhere.
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

Plain HTTP gives neither confidentiality nor authenticity. For a release, authenticity comes from its signature, which the updater checks (351-UPD-0005, 0007), not from the transport. `download` refuses `https://`. A program may now ask for the shell's TLS client (`REQUEST_TLS`, 351-KRN-0034), which the shell lends with a flow grant (351-APP-0015), but `download` does not use it yet. That is 351-NET-0002.

## Tested

These tests run in the `net` suite, on x86 and aarch64, in QEMU with user networking. They boot their own VM, and on x86 its boot disk is on AHCI: the IDE driver's port I/O, emulated, takes minutes for 30 MiB. 30 MiB takes about 90 s on x86 under TCG, almost all of it in writing the file. This is not a measure of throughput. On aarch64 the file is 8 MiB, cut at 3 MiB: there 30 MiB took more than 10 minutes. The server is `scripts/serve_release.py`, which now answers single byte ranges, over plain HTTP. It is given a hook that cuts the first response for a path.
- 30 MiB (8 MiB on aarch64) through `download`'s own grant: the first response is cut at 10 MiB (3 MiB), and the rest is asked for with `Range` from there.
- The SHA-256 is checked in the system. A second run finds the file complete (416) and changes nothing.
- A run given up on after one connection, at 50 000 of 200 000 bytes, is resumed by the next run.
- These are refused:
  - a missing file (`Status(404)`);
  - a port the grant does not name (`Denied` from the stack);
  - `https://`;
  - a file outside the writable directories.

## Not provided yet

- **HTTPS**, and trust for the update server: roots shipped with the release, or the server's key pinned (351-NET-0002).
- **Chunked bodies, redirects, keep-alive, several connections at once, IPv6.**
- **Fast writes of a large file.** `vfs_server` writes a file one 512-byte sector per block request, and walks the file's cluster chain from its start on every write. So writing slows down as the file grows. That is `vfs_server`'s, not this task's.
- **A parser with less authority than its program.** The head is parsed in the downloading program, which holds the grant and the file. Track D plans session parsers with minimal authority (Appendix B.6).
