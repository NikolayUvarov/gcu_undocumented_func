# HTTP, pinned servers and JSON for services

**Version:** 0.1 (2026-10-12) · **Track:** `NET`, tasks [750-NET-0019](../../issues/750-NET-0019-http-for-services.md) (HTTP), [750-NET-0020](../../issues/750-NET-0020-tls-with-two-pins.md) (pins) and [750-NET-0021](../../issues/750-NET-0021-json-in-the-parser-service.md) (JSON) for main tasks [750](../../issues/750-effector-agent.md) and [501](../../issues/501-effector.md) · **Roadmap:** track D · **Constitution:** MC-11.5, MC-11.11, MC-12.4, Appendix B.6

A service that talks to one server for as long as the machine runs, such as the Effector agent, needs more than `download` does. It posts JSON, reads chunked answers, keeps one connection, holds a long stream, trusts a server by a pin that can rotate, and has the JSON it receives read by a process that holds nothing (MC-11.11). This page describes what the network track gives it. The downloads guide ([downloads.md](downloads.md)) still describes `get` and `download`.

## `mind::http::Connection`

- **One transport for several requests.** `Connection::new(transport)` takes the caller's `Transport`: a TCP socket of its flow grant, or a session of the TLS service. So the network policy that applies to the caller applies to every request.
- **`request(method, url, headers, body, parser, close_delimited)`:**
  - sends `GET`, `POST` or `PUT` with `Host`, `User-Agent` and the caller's headers;
  - with a body, adds `Content-Length` and streams the body from a `Source`;
  - reads the response head, skipping 1xx answers, and has the `Parser` read it (the parser service, `mind::parse::Service`, as for downloads).
- **The caller's headers.** At most 16, in a request head of at most 4 KiB. A header is refused (`Header`) when its name is not a token, when its value holds anything but visible ASCII, spaces and tabs (no CR or LF, so nothing can be injected), or when it is one the library keeps: `Host`, `Content-Length`, `Connection`, `Transfer-Encoding`, `TE`, `Trailer`, `Upgrade`, `Keep-Alive`, `Proxy-Connection`, `Expect`.
- **`Response::read(out)`** gives the body in pieces, framed:
  - by `Content-Length`;
  - by chunks. A chunk-size line is at most 128 bytes with its extensions, which are ignored; trailers are at most 4 KiB and are not interpreted;
  - by the end of the connection, only when the caller passed `close_delimited` (otherwise `Length`);
  - 204 and 304 have no body.
- **`finish`** reads and drops the rest, so the connection can carry the next request.
- **When the connection closes.** `open()` is false after `Connection: close`, HTTP/1.0 without `keep-alive`, a close-delimited body, a framing error (`Framing`: a bare LF, a missing CRLF after a chunk, a chunk size of 16 digits or more, a line or trailers past their bounds) or a failed transport. The next request then needs a new connection (`Closed`).
- **Timeouts are the transport's,** so the caller chooses them. A wake-up stream that stays silent for 55 s is a `receive` that waits that long.
- **What the parser says.** `idl/parse.wit` 1.3 adds `http-response`, which is `http-head` with `close`. `mind::parse::Service` uses it. `http-head` stays for clients of 1.2.

## A server trusted by a pin (`idl/tls.wit` 1.2)

- **`connect-pins`** takes one or two pins. Each is the SHA-256 of the server's certificate (its DER, a *leaf* pin) or of its SubjectPublicKeyInfo (an *SPKI* pin). The second is the next one: a server matching either is accepted, so a pin can rotate without a moment when neither is right.
- **Still checked:**
  - the certificate's name against the one asked for (webpki);
  - its validity period by the system clock;
  - the server-authentication purpose when it names purposes;
  - `CA:false`;
  - the handshake's signature by its key.

  The certificate's issuer is not looked for: the pin is what is trusted, and a server may send its leaf alone.
- **Refused** (`certificate`), each logged by `tls`:
  - a server matching neither pin (`REFUSED: NOT A PINNED KEY OR CERTIFICATE`);
  - a wrong name (`REFUSED: CERTIFICATE NotValidForName`);
  - an expired or not yet valid certificate (`Expired`, `NotValidYet`);
  - a purpose other than server authentication, or a CA certificate (`InvalidPurpose`).
- **`connect-pinned` (1.1)** keeps its meaning: one SPKI pin and nothing else checked.
- **The pins of a certificate:**
  - leaf: `openssl x509 -in cert.pem -outform der | sha256sum`;
  - SPKI: `openssl x509 -in cert.pem -pubkey -noout | openssl pkey -pubin -outform der | sha256sum`.

## JSON read by the parser service (`idl/parse.wit` 1.4)

- **`json(text, start)`.** `parse` reads a whole JSON text (RFC 8259, UTF-8, up to 16 KiB) with `mind::json` and answers its values in document order, 16 at a time from `start`, with how many there are. Each value comes with:
  - its kind: object, array, string, number, true, false or null;
  - the index of the object or array it is in (65535 for the outermost);
  - its member's key (empty in an array);
  - its text: a string with its escapes decoded, a number as written (`-1.5e3` stays `-1.5e3`).
- **Bounds:**
  - 1024 values;
  - nesting 16 deep;
  - a key of 64 bytes and a string of 512 bytes once decoded.

  Anything past them, or malformed, is refused and logged with the client's PID (`[PARSE] REFUSED JSON FOR PID n`).
- **The client still checks the values against what it expects (MC-11.5):** which keys, of which kinds, and whether a key comes twice. `parse` does not decide what a document means.

## `webcheck`

The network track's diagnostic of all three, as a service would use them: `webcheck URL [--post JSON] [--repeat N] [--stream] [--idle SECONDS] [--pin leaf|spki:HEX]... [--name NAME] [--json]`. Over one connection, it:
- posts JSON with an `Authorization` header;
- makes N more requests;
- reads a stream to its end, up to `--idle` seconds without a byte;
- with neither, makes one GET.

With `--pin` it connects by `connect-pins`; with `--json` it prints a JSON body as `parse` reads it.

## Tested

The tests and their results are in each task's resolution: [750-NET-0019](../../issues/750-NET-0019-http-for-services.md), [750-NET-0020](../../issues/750-NET-0020-tls-with-two-pins.md), [750-NET-0021](../../issues/750-NET-0021-json-in-the-parser-service.md).

## Not provided

- **Redirects:** a 3xx is a status like any other, for the caller to read.
- **Several connections at once from one `Connection`**, pipelining, HTTP/2, IPv6.
- **Compression:** no `Accept-Encoding` is sent but the caller's.
- **More than 8 TLS sessions in the system** (the TLS service's limit).
- **A JSON writer** (the Effector track's 750-EFF-0004). JSON numbers are handed over as written; `parse` does not convert them.
