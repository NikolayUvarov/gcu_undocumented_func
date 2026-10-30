# 000-NET-0047 — HTTP for API clients in `libmind`: bodies, multipart, redirects, compression, a streaming body, Server-Sent Events

**Type:** network (`libmind` `http`) · **Owner:** `NET` · **Priority:** P1 · **Status:** open (landed in round 39; HEAD, PATCH and DELETE made 2026-10-30, their aarch64 run and landing left) · **Blocked by:** — · **Main task:** — (for 253, 255, 260, 261 and 000-NET-0048) · **Constitution:** MC-6.6, MC-11.5, MC-11.11, MC-12.1

Recorded by the network track on 2026-10-25, for the plan [docs/communications-and-agents.md](../docs/communications-and-agents.md).

## Problem

- **What `libmind`'s HTTP client does today:**
  - GET with a range and resume (351-NET-0001);
  - chunked bodies and keep-alive;
  - a few requests with bodies for services (750-NET-0019).
- **What the clients need:**
  - JSON POSTs with headers (every API);
  - `multipart/form-data` (Telegram's `sendDocument`, forms in `www`);
  - redirects (`www`, `get`);
  - `gzip` and `deflate` bodies;
  - a body read as it comes, without holding all of it;
  - **Server-Sent Events** (`text/event-stream`): every language model's API streams its answer this way;
  - long polling with a deadline (Telegram's `getUpdates`);
  - `Retry-After` on 429 and 503.

## Plan

- **A request** with any method, headers and a body, given whole or written as it is sent, with its `Content-Length` or chunked.
- **`multipart/form-data`:** a boundary that does not occur in the parts, a file part streamed from a VFS file.
- **Redirects:**
  - 301, 302, 303, 307 and 308 with their method rules;
  - at most 10, each reported to the caller;
  - never from HTTPS to HTTP;
  - a redirect to another host is followed only if the caller's flow grant covers it.
- **Compression:** `Content-Encoding: gzip` and `deflate` through `libmind` `inflate`, with a stated limit on the inflated size (a bomb stops there).
- **A streaming body:** the response's body read in pieces as it arrives, for large downloads and for SSE.
- **Server-Sent Events,** by the WHATWG rules: `event`, `data` over several lines, `id`, `retry`, and comments. Each event is handed to the caller whole, with a stated limit on its size.
- **Deadlines:** for the connection, for the head and between pieces of the body, each the caller's.
- **`Retry-After`:** parsed, in seconds or as a date, and given to the caller. The caller decides whether to repeat: nothing here repeats a request on its own (MC-6.6).
- **Parsing stays where it is:** the head and the framing here, a body's JSON or HTML in the caller's parser session (MC-11.11).

## What was done (2026-10-28)

- **`idl/parse.wit` 2.0.** One response head, `http-response`, now also carries:
  - the content coding;
  - the media type;
  - `Location`;
  - `Retry-After`, in seconds or as a date.

  1.x's `http-head` is gone (no older version kept, the maintainer's rule of 2026-10-25). `parse` and `mind::parse` follow.
- **`libmind` `http`:**
  - **The head.** `Head` has the new fields, and `Response::head` gives it whole. `http_date` reads the three HTTP-date forms.
  - **Redirects.** `redirect` applies the redirect rules: 303, and 301 or 302 after a POST, become GET; 307 and 308 keep the method and body. It refuses another scheme, HTTPS to HTTP, a missing `Location` and the eleventh. It tells the caller whether the host changed, since the caller's flow grant decides.
  - **Chunked uploads.** `request_streamed` sends a body of unknown length chunked.
- **`libmind` `http_coding`.** It decodes identity, gzip (members one after another) and deflate (zlib or raw) as the body arrives, up to the caller's limit (`TooLarge`).
  - **The readers.** `inflate` gains readers that ask for bytes as the stream needs them (`inflate_from`, `zlib_from`, `gzip_from`), and the gzip header and trailer. Its existing functions are unchanged.
- **`libmind` `multipart`.** RFC 7578 bodies of fields and streamed files, with the length known first.
  - **The boundary** is made from 16 random bytes.
  - **A field** that holds it is refused before anything is sent. **A file** is matched as it streams (Knuth-Morris-Pratt) and the request fails if the boundary occurs in it.
- **`libmind` `sse`.** Server-Sent Events by the WHATWG rules, fed in pieces of any size, with a limit on a line and on an event.
- **Host tests.**
  - **`tests/http_host.rs` (20 tests):** heads, dates, redirect rules and chunked uploads.
  - **`tests/sse_host.rs` (6 tests):** the standard's examples, every line ending, every split point and 1000 events.
  - **`tests/multipart_host.rs` (4 tests):** RFC 7578's form, the boundary found across reads, names that cannot be written, and a short file.
  - **`tests/http_coding_host.rs` (4 tests):** every coding in pieces down to one byte, a damaged CRC, an unknown coding, a bomb stopped at its limit, and a sink that stops.
  - **The fixtures** are written by `tests/data/http/make.py`.
- **The QEMU check.** `tests/http_api_stub.py` serves HTTPS with the suite's test CA, and `webcheck --api` runs each check against it in the download suite (`api_runs`):
  - a JSON POST;
  - a multipart upload that Python's own parser reads;
  - a chunked upload;
  - redirect chains;
  - gzip and deflate;
  - a 64 MiB bomb;
  - 1000 events;
  - `Retry-After` in both forms.
- **Docs.** [docs/network/services.md](../docs/network/services.md) 0.2.
- **Tested on x86** (QEMU under TCG, 4 CPUs, user networking): the download suite passed whole (27 checks), its `api_runs` among them, all on the test CA's HTTPS:
  - the JSON POST answered;
  - the multipart upload's 100 000 bytes arrived with the SHA-256 the guest computed;
  - the chunked upload's 50 000 bytes arrived;
  - four redirects (301, 308, 307, 302) then 200, and a 303 after a POST followed as a GET;
  - the loop refused after its 10th redirect, and the way down to HTTP refused;
  - gzip (12 630 bytes) and deflate (12 862) both decoded to the same 225 000 bytes;
  - the bomb stopped at 1 048 576 bytes;
  - 1000 events in order, the last id 999 and `retry` 3000;
  - `Retry-After` as 7 seconds, and as the date 1445412480.
- **Tested on aarch64** (QEMU under TCG, 4 CPUs): the `tls` and `net` suites passed whole (29 checks), the API clients' among them, with the same lines.
- **Left:** `Head`, `Patch` and `Delete` wait for the Effector agent to stop matching `Method` exhaustively (`requests-EFF.md`, 2026-10-28).

## Acceptance criteria

- **Host tests:** each framing (length, chunked, close), multipart's boundary, every redirect rule, a gzip bomb stopped, SSE split at every byte boundary, `Retry-After` in both forms.
- **In QEMU, x86 and aarch64:** a stand-in server (`tests/http_api_stub.py`, HTTPS with the suite's test CA) answers a JSON POST, a multipart upload, a redirect chain, a gzip body and an SSE stream of 1000 events. A test program checks each.

## Progress (2026-10-28)

Landed in round 39 (`main` 8721ef86, the gate on its code 381d1e85): `parse.wit` 2.0, redirects, content codings, multipart bodies, Server-Sent Events, chunked uploads and Retry-After, with `webcheck --api` against the stand-in in the `tls` suite, x86 and aarch64, and the host tests `http`, `sse`, `multipart` and `http_coding`. Left: HEAD, PATCH and DELETE checked against the stand-in.

## Progress (2026-10-30): HEAD, PATCH and DELETE

On `claude/youthful-mendel-mf1soy` (f7fd71f5, 240b4f71). The Effector agent no longer matches `Method` exhaustively (000-EFF-0019), so nothing waited.

- **`mind::http`:**
  - `Method` gains `Head`, `Patch` and `Delete`, and `Method::parse` reads a name, in capitals only;
  - the answer to HEAD ends at its head, whatever length or chunked coding it names, as 204 and 304 do (RFC 9112 6.3). Its head's `length` is what a GET would get;
  - a 303 keeps HEAD, while any other method becomes GET.
- **`get`:**
  - `-I` sends HEAD;
  - `-X` takes HEAD, PATCH and DELETE. `-X HEAD` shows the head as `-I` does;
  - `-X TRACE` is refused (usage, 2).
- **The stand-in** (`tests/http_api_stub.py`):
  - it answers HEAD with GET's head and no body;
  - PATCH as PUT;
  - DELETE: the echo, and 204 for `/api/item`.
- **`webcheck --api`** adds a run on one connection:
  - HEAD of `/api/echo`, a length named;
  - HEAD of `/api/events`, chunked named;
  - a GET after them, answered whole;
  - PATCH with a merge-patch body;
  - DELETE of the echo, and DELETE of `/api/item`, answered 204;
  - six answers served on the one connection.
- **The suite:** `--suites download` runs the net suite's download checks alone (`download_check`).
- **Host tests:**
  - `tests/http_host.rs`: HEAD with a length and with chunked, then PATCH and DELETE, on one connection; the exact requests sent; 303 and 301 with HEAD and DELETE; the names both ways (21 tests pass);
  - `tests/get_host.rs`: `-I`, `-X head`, `-X PATCH`, `-X DELETE`; `-X TRACE` and `-X HEAD -d` refused.
- **Docs:** [docs/network/services.md](../docs/network/services.md), [docs/network/downloads.md](../docs/network/downloads.md).
- **x86, QEMU under TCG** (4 CPUs, locally): `--suites download` passed whole, 14 checks, with the new lines, and `get`'s `-X PATCH`, `-X DELETE` and `-X DELETE -I` (204).

Left: the same run on aarch64, and landing.

## Related

[351-NET-0001](../issues-done/351-NET-0001-http-downloads.done), [750-NET-0019](../issues-done/750-NET-0019-http-for-services.done), [000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md), [260](260-ai-agents-on-the-platform.md).
