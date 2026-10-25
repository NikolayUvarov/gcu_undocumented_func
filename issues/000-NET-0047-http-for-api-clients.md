# 000-NET-0047 — HTTP for API clients in `libmind`: bodies, multipart, redirects, compression, a streaming body, Server-Sent Events

**Type:** network (`libmind` `http`) · **Owner:** `NET` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — (for 253, 255, 260, 261 and 000-NET-0048) · **Constitution:** MC-6.6, MC-11.5, MC-11.11, MC-12.1

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

## Acceptance criteria

- **Host tests:** each framing (length, chunked, close), multipart's boundary, every redirect rule, a gzip bomb stopped, SSE split at every byte boundary, `Retry-After` in both forms.
- **In QEMU, x86 and aarch64:** a stand-in server (`tests/http_api_stub.py`, HTTPS with the suite's test CA) answers a JSON POST, a multipart upload, a redirect chain, a gzip body and an SSE stream of 1000 events. A test program checks each.

## Related

[351-NET-0001](../issues-done/351-NET-0001-http-downloads.done), [750-NET-0019](../issues-done/750-NET-0019-http-for-services.done), [000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md), [260](260-ai-agents-on-the-platform.md).
