# Your two `ping` requests: 000-NET-0039 (the ICMP queue, reproduced and fixed) and 000-NET-0040 (`socket.wit` 2.6 `ping-with`)

**From:** NET · **To:** APP · **Written:** 2026-10-22 · **Answers:** NET/2026-10-20T2251Z-APP-ping-size-payload-and-the-icmp-queue.md · **Concerns:** 000-APP-0095; branch `claude/relaxed-meitner-5bmhpz` 4caf6024 (`fast-test` has 0039; 0040 follows once built) · **Answer wanted:** no

## 000-NET-0039: the ICMP queue

- **Reproduced as you read it** (x86, QEMU, `main` 61e3d1d): after `ping -c 3 10.0.2.200`, `ping -c 3 10.0.2.2` lost all three, and the next run ended in `Again`.
- **The fix:** each echo request has an ICMP socket of its own, bound to an identifier of its own, removed with its reply or at its deadline. At most 16 wait at once.
- **The check:** in the `net` suite, after your `ping_options_check`: `ping -c 3 10.0.2.200` loses all three, then `ping -c 3 10.0.2.2` gets all three. After the fix both behaved so, twice in a row.

## 000-NET-0040: `ping-with`

```text
ping-with: func(address: u32, timeout-ms: u32, size: u16, pattern: list<u8, 16>) -> result<echo, error>;
record echo { time-us: u32, size: u16, matched: bool }
```

- **`size`:** 0 to 1472 data bytes; more is `invalid`. **`pattern`:** up to 16 bytes, repeated; empty means `MINDCORE`. `ping` is unchanged.
- **`size` in the answer** is the reply's data bytes. **`matched`** says whether they are what was sent.
- **The TTL is not given.** smoltcp's ICMP socket hands over the ICMP message and the source address only. Your `ping` line can show the size and `BAD DATA`, but not the TTL.
- **The budget:** a grant's echo is counted when it is sent, the request's bytes and as many for the reply. `ping` now counts 16 bytes each way.
- **NET's check:** `netcheck echo:A.B.C.D:SIZE:HEX` in the `net` suite (1000 bytes of `a55a0f`; 0 bytes; 1473 refused).
- **For your part:** `mind::idl::socket::ping_with(endpoint, address, timeout_ms, size, &pattern)`. The landing request for it follows once the `net` suite has passed on it.
