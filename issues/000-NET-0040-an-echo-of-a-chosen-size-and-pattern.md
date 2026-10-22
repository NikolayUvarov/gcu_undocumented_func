# 000-NET-0040 — An echo request of a chosen size and pattern, its reply's size and data checked (`socket.wit` 2.6)

**Type:** network (stack, interface) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P3 · **Status:** in progress (built; the `net` suite's check written) · **Blocked by:** — · **Main task:** — (for [000-APP-0095](000-APP-0095-ping-options.md)) · **Roadmap:** track D · **Constitution:** MC-11.6, MC-12.4

From the tools track's request "An echo request's size and payload, for `ping -s` and `-p` (000-APP-0095)" in requests-NET.md (2026-10-20), which this task takes from that file.

## Problem

- `socket.wit` `ping(address, timeout-ms)` sends 8 bytes of data, `MINDCORE`. The caller cannot choose the size or the payload.
- The reply is matched by its sequence alone. Its data are not compared with what was sent, and its size is not told.
- So the shell's `ping` has `-c`, `-i`, `-w` and `-f` (000-APP-0095), but `-s size` and `-p pattern` wait for the stack.

## Plan

- **`socket.wit` 2.6:** `ping-with(address, timeout-ms, size, pattern) -> result<echo, error>`, with `record echo { time-us, size, matched }`.
  - `size` is 0 to 1472 data bytes, one frame on a 1500-byte MTU; more is `invalid`.
  - `pattern` is 1 to 16 bytes repeated over the data; empty means the stack's own, `MINDCORE`.
  - The answer gives the round trip, the reply's data size, and whether its data are what was sent.
  - `ping` stays as it was: 8 bytes of `MINDCORE`.
- **The budget.** A grant's echo is counted in its volume when it is sent: the request's bytes, and as many for the reply. The stack's policy object is not reached when a reply arrives, so the reply is counted ahead. `ping` is now counted the same way, 16 bytes each way.
- **Not given: the reply's TTL.** smoltcp's ICMP socket hands over the ICMP message and the source address only, not the IP header. Telling the TTL would take a raw socket per request that builds its own IP header, or a change in smoltcp. It is left out, and the tools track is told.
- **A check, `netcheck echo:A.B.C.D:SIZE:HEX`,** in the `net` suite:
  - 1000 bytes of `a55a0f` come back whole and the same;
  - 0 bytes with the stack's own pattern come back;
  - 1473 bytes are refused (`Invalid`).
- **The tools track then** gives the shell's `ping -s` and `-p` through it.

## Acceptance criteria

- [ ] The `net` suite's echo check passes on x86 and aarch64.
- [ ] The IDL fuzzer covers `echo` and `ping-with` (done on the host: `tests/idl_fuzz_host.rs`).

## Related

- [000-APP-0095](000-APP-0095-ping-options.md): `ping`'s options.
- [000-NET-0039](000-NET-0039-an-echo-request-holds-no-other.md): an ICMP socket per echo request, which this builds on.
