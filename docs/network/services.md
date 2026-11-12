# HTTP, pinned servers and JSON for services

**Version:** 0.2 (2026-10-28) · **Track:** `NET`, tasks [750-NET-0019](../../issues-done/750-NET-0019-http-for-services.done) (HTTP), [750-NET-0020](../../issues-done/750-NET-0020-tls-with-two-pins.done) (pins), [750-NET-0021](../../issues-done/750-NET-0021-json-in-the-parser-service.done) (JSON) and [000-NET-0047](../../issues-done/000-NET-0047-http-for-api-clients.done) (API clients) for main tasks [750](../../issues/750-effector-agent.md) and [501](../../issues/501-effector.md) · **Roadmap:** track D · **Constitution:** MC-11.5, MC-11.11, MC-12.4, Appendix B.6

A service that talks to one server for as long as the machine runs, such as the Effector agent, needs more than `download` does. It posts JSON, reads chunked answers, keeps one connection, holds a long stream, trusts a server by a pin that can rotate, and has the JSON it receives read by a process that holds nothing (MC-11.11). This page describes what the network track gives it. The downloads guide ([downloads.md](downloads.md)) still describes `get` and `download`.

## `mind::http::Connection`

- **One transport for several requests.** `Connection::new(transport)` takes the caller's `Transport`: a TCP socket of its flow grant, or a session of the TLS service. So the network policy that applies to the caller applies to every request.
- **`request(method, url, headers, body, parser, close_delimited)`:**
  - sends `GET`, `POST`, `PUT`, `HEAD`, `PATCH` or `DELETE` with `Host`, `User-Agent` and the caller's headers (`Method::parse` reads a name, in capitals);
  - with a body, adds `Content-Length` and streams the body from a `Source`;
  - reads the response head, skipping 1xx answers, and has the `Parser` read it (the parser service, `mind::parse::Service`, as for downloads);
  - ends the answer to `HEAD`, and a 204 or 304, at its head, whatever length or coding it names: the head's `length` is what a GET would get, and the connection serves the next request (000-NET-0047).
- **The caller's headers.** At most 16, in a request head of at most 4 KiB. A header is refused (`Header`) when its name is not a token, when its value holds anything but visible ASCII, spaces and tabs (no CR or LF, so nothing can be injected), or when it is one the library keeps: `Host`, `Content-Length`, `Connection`, `Transfer-Encoding`, `TE`, `Trailer`, `Upgrade`, `Keep-Alive`, `Proxy-Connection`, `Expect`.
- **`Response::read(out)`** gives the body in pieces, framed:
  - by `Content-Length`;
  - by chunks. A chunk-size line is at most 128 bytes with its extensions, which are ignored; trailers are at most 4 KiB and are not interpreted;
  - by the end of the connection, only when the caller passed `close_delimited` (otherwise `Length`);
  - 204 and 304 have no body.
- **`finish`** reads and drops the rest, so the connection can carry the next request.
- **When the connection closes.** `open()` is false after `Connection: close`, HTTP/1.0 without `keep-alive`, a close-delimited body, a framing error (`Framing`: a bare LF, a missing CRLF after a chunk, a chunk size of 16 digits or more, a line or trailers past their bounds) or a failed transport. The next request then needs a new connection (`Closed`).
- **Timeouts are the transport's,** so the caller chooses them. A wake-up stream that stays silent for 55 s is a `receive` that waits that long.
- **What the parser says.** `idl/parse.wit` 2.0 has one head, `http-response`: the status, length, range, chunked and `close`, and (000-NET-0047) the content coding, the media type, `Location` and `Retry-After`. 1.x's `http-head` is gone. `mind::parse::Service` uses it, and `Response::head` gives it whole.
- **A body of unknown length** goes chunked with `request_streamed(method, url, headers, source, …)`: each piece the source gives is one chunk (at most 4 KiB), until it gives 0.

## For API clients (000-NET-0047)

- **Redirects: `mind::http::redirect(from, method, head, followed, out)`.** It reads a 301, 302, 303, 307 or 308 with its `Location`, resolves it against `from` into the caller's buffer, and says:
  - the method and whether the body goes again: 303 makes any method but HEAD a GET without a body, and so do 301 and 302 for a POST, as browsers do; 307 and 308 keep both;
  - whether the next URL is on the same host and port.

  It refuses (`Redirect`) a redirect without a `Location`, one to another scheme, one from HTTPS to HTTP, and the eleventh (`REDIRECTS_MAX`, 10). The caller follows: a new host needs its own flow grant, which the library cannot give. A fragment is dropped.
- **Coded bodies: `mind::http_coding::decode(response, limit, sink)`.**
  - It decodes `identity`, `gzip` and `x-gzip` (one member or several), and `deflate` (zlib, or the raw stream some servers send, told apart by the first two bytes).
  - It decodes as the body arrives (`mind::inflate`'s readers), into the sink in pieces, never holding the body whole.
  - It stops at `limit` decoded bytes with `TooLarge`, so a bomb ends there.
  - A damaged stream, a bad CRC-32 or length, and any other coding fail with `Encoding`.
  - `Accept-Encoding` is the caller's to send.
- **Multipart bodies: `mind::multipart::Multipart`** (RFC 7578) is a `Source` of fields and files.
  - Its length is known before the first byte, so it goes with `Content-Length`.
  - The boundary is `MIND-` and 32 hex digits from 16 random bytes the caller gives.
  - A field that holds the boundary is refused before anything is sent. A file is checked as it streams, and the request fails with `Body` if the boundary occurs in it.
  - Names and file names are quoted strings: no quote, backslash or control character.
- **Server-Sent Events: `mind::sse::Stream`** reads `text/event-stream` by the WHATWG rules:
  - the bytes are fed as they come, in pieces of any size;
  - every line end is taken (CR LF, LF or CR, across pieces), a byte order mark at the start is dropped, and comments are skipped;
  - `event`, `data` over several lines, `id` (without NUL) and `retry` are read, and other fields are ignored;
  - each event is handed over whole;
  - a line or an event's data over the caller's limit gives up the stream (`TooLarge`).

  Reconnecting is the caller's, with `retry()` and `last_id()`.
- **The head's other fields:**
  - `head.encoding`;
  - `head.media`: `Content-Type`'s media type in lower case;
  - `head.location`;
  - `head.retry_after`: `Seconds(n)`, or `Date(t)` from an HTTP-date in any of its three forms (`mind::http::http_date`).

  An invalid `Retry-After` is ignored. Nothing here repeats a request on its own (MC-6.6).
- **Deadlines are the transport's,** as above. `webcheck`'s transport waits for each piece up to its idle time, so a stalled connection, head or body ends there.

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

## The server role (`idl/tls.wit` 1.3)

A client serves TLS 1.3 as the device (110-NET-0069). The release server `httpd` is the first that does ([docs/update/server.md](../update/server.md)).

- **`listen(session, port, backlog)`:** the attached session listens on `port` of the flow the client lent. That flow's listening grant names the port and the networks a peer may connect from (`socket.wit` 2.9, the policy's `listen` line).
- **`accept(listener)`:** the oldest connection a peer set up, as a new session of the same client, whose handshake as the server has started. It answers `again` when none waits, and `limit` when the client holds 8 sessions or the service 16 (the connection waits in the backlog).
- **`served(session)`:** the handshake taken as far as the peer's bytes allow. It answers `again` while it waits for the peer; the suite and group once it is done; `timeout` when it was not done within 10 s of its accept; `handshake` when it failed otherwise. `send` and `receive` answer `again` until it is done.
- **No call waits for a peer.** The service runs one request at a time, so a slow peer would otherwise hold up every other session.
- **The certificate:**
  - the device certificate (Ed25519, self-signed); or
  - the chain in `tlsserver.pem` on the boot disk, whose first certificate must hold the device key (else `listen` answers `certificate`).

  Each signature is `keystore`'s (`keystore.wit` 1.7 `tls-server`, at most 120 a minute): the private key never enters `tls`.
- **Who may serve:** a client with the right to the device certificate. One badged `BADGE_TLS_NO_DEVICE`, the copy the shell lends a program, is denied `listen`, and `tls` logs it.
- **Not provided:** TLS 1.2, resumption (no tickets), client certificates, ALPN, and a key other than the device's.

## The SSH server (`sshd`, 112-NET-0072)

`sshd` takes SSH logins and file copies (OpenSSH's `scp` since 9.0, and `sftp`). The protocol is the `ssh` crate's server role (112-NET-0071): curve25519-sha256 with strict key exchange, chacha20-poly1305@openssh.com, an ssh-ed25519 host key, public keys only.

- **Off by default.** `init` starts it only when the policy gives program `sshd` a flow, whose listening grant names TCP 22: `listen sshd tcp 22 from 192.168.1.0/24`. It listens only once its keys file lists a key.
- **The host key is the device key.** `keystore` signs each key exchange's hash (`keystore.wit` 1.8 `ssh-host`, at most 60 a minute); the key never enters `sshd`. Its fingerprint is in the log: `[SSHD] HOST KEY: THE DEVICE KEY, SHA256:…`, and `keystore` logs the key in the `known_hosts` form at boot.
- **The owner's keys: `data/ssh/authorized_keys`,** read at each connection. Each line names what its key may do, then the key as OpenSSH writes it:

  ```text
  files ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI… owner@laptop
  console,files ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI… owner@desktop
  ```

  A line without a right (`console`, `files` or both), with another key type or an OpenSSH option is left out, and the log names its number and why. The user is `mind`: `ssh mind@<machine>`.
- **Files:** SFTP version 3 over `data/`; `data/ssh/` is refused, so the files' right never reaches another key. `scp -s` or OpenSSH 9's `scp`, and `sftp`: read, write, list, make and remove directories, rename, remove.
- **The console:** the shell's commands, through the shell's client `init` lends badged `BADGE_SHELL_REMOTE`, on a remote session's rule (112-APP-0136): what the link's developer client runs at once (`ps`, `kill`, `logs`, `date set`, `ping`, …); refused are the policy's changes, `mode`, scripts, the screen's commands, programs, `update`, `passphrase`, `boot`, `tpm` and a line with `>` or `>>` (files are the `files` right's). Nothing it runs asks at the machine. `ssh mind@<machine> ps` lists the processes; the shell logs each line it ran or refused for an SSH session.
- **Limits:** one session at a time; a login within 30 s; 10 min of silence; 10 connections a minute from one address; 1 GiB written and 8 open files a session.
- **The log:** each login with its key's fingerprint and comment, each refusal and why, each console command's word, each file written or removed, each session's end. No session's data.
- **Not provided:** passwords, keyboard-interactive, port forwarding, an agent, X11, the old SCP protocol (`scp -O`), rekeying (a client's rekey ends the session; OpenSSH asks for one after some gigabytes).

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

With `--pin` it connects by `connect-pins`; with `--json` it prints a JSON body as `parse` reads it. With `--api` it runs the API clients' checks against `tests/http_api_stub.py` (000-NET-0047), one connection each, one line per step.

## Tested

The tests and their results are in each task's resolution: [750-NET-0019](../../issues-done/750-NET-0019-http-for-services.done), [750-NET-0020](../../issues-done/750-NET-0020-tls-with-two-pins.done), [750-NET-0021](../../issues-done/750-NET-0021-json-in-the-parser-service.done), [000-NET-0047](../../issues-done/000-NET-0047-http-for-api-clients.done).

## Not provided

- **Methods other than GET, POST, PUT, HEAD, PATCH and DELETE** (OPTIONS, TRACE, CONNECT).
- **Several connections at once from one `Connection`**, pipelining, HTTP/2, IPv6.
- **Content codings other than gzip and deflate** (`br`, `zstd`): refused with `Encoding`.
- **More than 8 TLS sessions in the system** (the TLS service's limit).
- **A JSON writer** (the Effector track's 750-EFF-0004). JSON numbers are handed over as written; `parse` does not convert them.
