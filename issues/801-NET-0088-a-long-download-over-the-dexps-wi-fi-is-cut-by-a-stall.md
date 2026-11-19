# 801-NET-0088 — A long download over the DEXP's Wi-Fi is cut by a 10 s stall, and runs at an 8 KiB window

**Type:** `netstack` · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** open (2026-11-19) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S7, S8) · **Roadmap:** stage V · **Constitution:** MC-9.3, MC-12.1

## Problem

On 2026-11-19 the DEXP took releases of about 29 MB through Effector's `update_agent` (801-KRN-0258, 801-KRN-0259). Six of its ten archive downloads failed.

**How they failed** (`/api/commands/file BODY: Transport`, the command `upload`):
- they were cut after 184 to 439 s;
- the agent then said `THE SERVER ANSWERS AGAIN, AFTER n S`;
- the four taken lasted about 9 minutes each.

**What the log ring shows around a cut** (`log:/` ring, 13:00 and 13:30 UTC):
- The radio's ten-second counters run at about 1800 frames received and 390 sent, all acknowledged, none dropped.
- In the last ten seconds before each cut they fall to 769, 1377 or 1078. The agent's read of a body waits 10 s for a byte (`effector/src/main.rs`, `Link::receive`, `REQUEST_MS`), so the stream stopped for 10 s.
- The radio counted no frame lost. Where the stall comes from is not known: TCP's recovery after a loss, the server or the path.

**The speed.** 29 MB in 9 minutes is about 54 KB/s, on a link the station joined at 24 Mb/s. `netstack` gives each TCP socket an 8 KiB receive buffer (`TCP_BUFFER`), so at most 8 KiB are in flight. 8 KiB a round trip is 54 KB/s at a round trip of about 150 ms, near enough to explain the speed, though the round trip is not measured. A window of five or six segments also gives few duplicate acknowledgements after a loss, so the server waits for its retransmission timer more often.

The agent starts a cut download again from its first byte. Every try then needs its full 9 minutes with no stall of 10 s.

## Plan

1. **`netstack`:** a receive buffer of 64 KiB for a TCP connection a client opens, within the service's memory; the send buffer stays as it is. To measure:
   - the `netbench` suite's speed before and after (QEMU, where the round trip is short);
   - the DEXP's download time.
2. **A stall named where it happens:** when a TCP connection's receive has seen nothing for 5 s, while data is still owed, `netstack` says so once, with smoltcp's state, window and the last sequence numbers. That way the next cut shows whether the window was closed or nothing arrived.
3. **EFF's part,** recorded in `issues/requests-EFF.md`, not done here:
   - a cut archive download goes on from the bytes the gateway holds, with a `Range` request, within the command's time;
   - a body read waits longer than the 10 s bound of a short request.
4. **The DEXP:** five downloads of a 29 MB release in a row, each taken, with their times.

## Acceptance criteria

- [ ] `netstack`'s TCP receive buffer is 64 KiB; the `net`, `download`, `tls` and `netbench` suites pass on x86 and aarch64, `netbench`'s speed before and after named.
- [ ] A stalled receive is named in the log with the socket's state.
- [ ] On the DEXP, five downloads of a 29 MB release are taken in a row, with their times (MC-12.1: that machine, that network).

## Related

[801](801-the-bench-stays-manageable.md), [801-KRN-0259](801-KRN-0259-the-pchs-over-clocking-watchdog.md) (the runs, on its branch), [requests-EFF.md](requests-EFF.md).
