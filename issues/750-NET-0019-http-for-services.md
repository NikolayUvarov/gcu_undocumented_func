# 750-NET-0019 — HTTP/1.1 for services, over one kept connection

**Type:** network (library) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) (with 501) · **Roadmap:** track D · **Constitution:** MC-11.5, MC-11.11, Appendix B.6

The HTTP part of the request in requests-NET.md ("HTTP for a service, a certificate pin and long-lived sessions", 2026-10-09). The pins are [750-NET-0020](750-NET-0020-tls-with-two-pins.md), JSON in the parser service [750-NET-0021](750-NET-0021-json-in-the-parser-service.md).

## Problem

`mind::http` makes one GET per connection, with fixed headers and `Connection: close`, for `download`. The Effector agent needs more:
- **Requests:** POST and PUT with JSON bodies and an `Authorization` header.
- **Responses:** chunked bodies (a Go server's answer once it outgrows its buffer).
- **One connection kept** for a heartbeat every second.
- **A wake-up stream** held open for up to 55 s.

## Plan

- **`mind::http::Connection`:** one transport kept for several requests.
  - **`request`** sends `GET`, `POST` or `PUT` with `Host`, the caller's headers and a body of known length streamed from a `Source`, then reads the response head (1xx answers skipped) and has the `Parser` read it.
  - **The caller's headers** are refused if they are not a header (a name that is not a token, a CR or LF in a value) or one the library keeps (`Host`, `Content-Length`, `Connection`, `Transfer-Encoding`, `TE`, `Trailer`, `Upgrade`, `Keep-Alive`, `Expect`, …). At most 16 of them, in a head of at most 4 KiB.
  - **`Response::read`** gives the body in pieces, framed by `Content-Length`, chunked, or (only when the caller allows it) the end of the connection. 204 and 304 have none.
  - **Bounds.** A chunk-size line is at most 128 bytes with its extensions, and trailers at most 4 KiB; neither is interpreted. A bare LF, a missing CRLF or a chunk size of 16 digits ends the connection (`Framing`).
  - **`finish`** drops the rest, so the connection serves the next request.
  - **The connection closes** after `Connection: close`, HTTP/1.0 without `keep-alive`, a close-delimited body or any failure; `open()` says so.
- **`Head.close`**, and **`idl/parse.wit` 1.3** `http-response`: the parser service now says whether the connection closes. `http-head` stays for 1.2's clients.
- **The idle timeout is the transport's,** so the caller chooses it: a stream that waits 55 s is a receive that waits.
- **`webcheck`,** the network track's check, does what a service would: a JSON POST, N requests on one connection, a long stream.

## Acceptance criteria

- **Host tests** (`tests/http_host.rs`):
  - requests with bodies on one connection;
  - 101 requests on one connection;
  - chunked bodies with extensions and trailers, read in pieces of 1 to 64 bytes;
  - every framing bound;
  - how a connection ends;
  - the headers a caller cannot send;
  - a body source that ends early.
- **In QEMU** (the `net` suite's download check, x86 and aarch64), over one TLS session of the TLS service to a test server:
  - a JSON POST with an `Authorization` header, answered chunked;
  - 100 more requests on the same connection;
  - a stream the server holds for 55 s before its body, read to its end.

## Related

[docs/network/downloads.md](../docs/network/downloads.md), [351-NET-0001](../issues-done/351-NET-0001-http-downloads.done), [109-NET-0008](../issues-done/109-NET-0008-parser-service.done).
