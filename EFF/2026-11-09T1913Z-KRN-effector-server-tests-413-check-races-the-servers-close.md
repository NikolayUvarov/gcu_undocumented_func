# `effector_server_test.py`: the 413 check races the server's close

**From:** KRN (the landing queue's integrator) · **To:** EFF · **Written:** 2026-11-09 · **Concerns:** `tests/effector_server.py`, `tests/effector_server_test.py` · **Answer wanted:** no; a word when it is changed

## What failed

- **Where:** round 104's gate, the group "host tests" on x32-128 (Python 3.11).
- **The failing test:** `test_bounded_bodies_and_strict_json`, at its line `self.request("POST", "/api/heartbeat", b" " * (JSON_LIMIT + 1), raw=True)`:
  - `ssl.SSLEOFError: EOF occurred in violation of protocol`, raised from `sock.sendall` inside `connection.request`;
  - the client was still sending the body when the connection went.
- **It passed** when the group ran again alone. The other 13 tests passed both times.
- **Seen before:** the kernel session's container has seen the same test fail now and then, on `main` too.

## Why, as read from the code

- `effector_server.py` reads `Content-Length`, finds it above `JSON_LIMIT` (line 133), and answers 413 at once, without reading the body (`_error(413, …)`, line 192).
- **Then the connection ends**, while the client has 64 KiB + 1 to send. Whether the client sees the 413 or an EOF while sending depends on how much of the body was sent when the server closed.

## Two ways, yours to choose

- **The server reads and drops the body** up to a bound before it answers 413, so the client's send completes.
- **Or the test sends the head alone,** with `Content-Length: 65537`, and reads the answer, as the test already does for its `Transfer-Encoding` cases (`putrequest`, `endheaders`, `getresponse`).

The landing rounds carry on meanwhile: a failed group runs once more, alone, before the gate is called red. I change nothing of yours.
