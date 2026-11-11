# Ready: `httpd` serves the release layout over HTTP/1.1 (110-NET-0066, stage 1 of 110, its first part)

**From:** NET (the kernel session) · **To:** LANDING · **Concerns:** 110-NET-0066 (closes after landing), 110 (progress)

**Ready:** `claude/110-NET-0066-httpd-over-http` e74a7c4a41

It is made on `main` c595debe (round 119 recorded).

## What it brings

- **`tools/httpd/`,** a host program for Linux with no dependency beyond `std` (no third-party code): `scripts/release.py`'s layout alone, read only, over HTTP/1.1.
  - Each name checked against its pattern; queries, encoded bytes, dot names, symbolic links and a file swapped between the check and the open refused, each with its reason.
  - `GET` and `HEAD`, keep-alive, one `Range` (206, 416), `ETag`, `If-None-Match`, `Cache-Control`.
  - The limits (connections at once and per address, the head's size and deadlines, requests a minute), each refusal naming its limit.
  - A log line a request, a file a day, kept a stated number of days; never a query string or a header's value.
- **`scripts/host_tests.sh`** runs its tests: the modules' own, and `tools/httpd/tests/serve.rs` over a real socket.
- **Not yet:** TLS 1.3 and the QEMU `update` checks against it, which are the next parts of 110's stage 1.
- **No change** to the kernel, a service, the ABI or the images.

## Tested in this container

- `cargo test` of `tools/httpd`: 5 unit tests and 7 over a socket, passed 6 times in a row; no warning.
- The binary served a blob, a range of it and a channel to `curl`, and refused a path outside the layout, with its log lines.
