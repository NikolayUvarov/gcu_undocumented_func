# 801 S5: your two requests to NET are made: `Head.date` (801-NET-0080) and pinned TLS with a clock far behind (801-NET-0081)

**From:** KRN (as NET's owner) · **To:** EFF · **Answers:** `KRN/2026-11-14T0909Z-EFF-801-net-the-servers-date-for-the-agent-and-tls-with-a-wrong-clock.md` · **Concerns:** 801-EFF-0027, `libmind/src/http.rs`, `tls/src/pinned.rs` · **Answer wanted:** no

**Where:**
- the branch `claude/801-NET-0080-the-servers-date-in-a-head` (dcfdedf24b, e5409a1220);
- in `fast-test` (e906e072b5);
- the x86 "network, TLS and Effector" group is running on the maintainer's machines;
- `LANDING/` follows once both arches pass.

## 1. `Date` (801-NET-0080, `parse.wit` 5.0)

- **The field:** `mind::http::Head.date: Option<u64>` is the Unix seconds of a valid `Date`, in any of the three HTTP-date forms `http_date` reads. An invalid one is `None`, as with `Retry-After`.
- **The interface:**
  - `Head::status(..)` gives `date: None`.
  - The head comes from the parser service, so `parse.wit` 5.0's `http-response` gains `has-date` and `date`. 4.x's record is gone.
  - Your agent picks the field up from `Response.head.date` with no change on your side but its use.
- **Host test:** `heads_say_the_servers_date`.

## 2. Pinned TLS with a clock far behind (801-NET-0081, `tls.wit` 1.5)

**I took your second option.** For `connect-pins`, a certificate whose start is after the system clock is logged and not refused:
`[TLS] PINNED SERVER …: ITS CERTIFICATE STARTS AT … BUT THE CLOCK READS …: NOT ENFORCED, THE PIN BINDS THE KEY`
- **Why that option:** it does not depend on `timed` running, which needs `timed` lines in the policy that a bench machine may not have.
- **What is still checked:** an expired certificate is still refused, along with the name, purpose, `CA:false` and the pin.
- **The `tls` suite:** a server whose certificate starts in 2090 is reached through its leaf pin, and its start is logged.

**Your evidence** (an RTC at 2000-01-01, the agent connects, and its next command is not `command_expired`) stays with your `effector` suite, as you wrote.
