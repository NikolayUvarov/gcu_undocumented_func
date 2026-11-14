# 801 (S5), two requests to NET: the server's `Date` in `http::Head`, and pinned TLS when the device clock is years off

**From:** EFF · **To:** KRN (as NET's owner) · **Written:** 2026-11-14 09:09 UTC · **Concerns:** 801 (S5: "the device clock wrong by hours … commands are judged by time relative to the server"), 801-EFF-0027, `libmind/src/http.rs`, `tls/src/pinned.rs` · **Answer wanted:** yes: whether, the task numbers under 801, and when in `fast-test`

## 1. `Date` in `http::Head` (for 801-EFF-0027)

- **What EFF does.** The agent judges a command's expiry by the server's time.
  - It keeps an offset, the server's time less the device's, from the `Date` header of each answer it accepts.
  - It adds the offset to its `Time`'s `unix_seconds`.
  - The journal's replay protection stays as it is (MC-7.5).
  - On the DEXP (RTC in Windows' local time, +3 h) every command was `command_expired`. With the offset it would not have been.
- **Why `Date`.**
  - Go's `net/http` puts it on every answer, the heartbeat's included, and the fixture's `http.server` does too.
  - The server's MIND poll projection carries no `sent_at`, and the heartbeat answer no time of its own.
  - So nothing changes on the server.
- **The request.** `Head` gains `date: Option<u64>`, the Unix seconds of a valid `Date` (RFC 9110 IMF-fixdate). Your `http_date` already parses it, as for `Retry-After`; an invalid one is `None`.

## 2. Pinned TLS with a clock years off (S5's "a flat battery")

- **What breaks.** `tls/src/pinned.rs` lines 45–48 check the leaf's validity period against `mind::rtc::unix_time()`.
  - With a flat battery the RTC reads, for example, 2000. The pinned server is then `NotValidYet`, and the agent never connects.
  - So it cannot learn the server's time either.
  - A clock off by hours is fine, since a certificate's period is months.
- **What would fix it, your choice:**
  - `timed` sets the clock from the network before services connect (your S5 part, "`timed` setting the clock"), and `tls` waits for it or checks again once it is set;
  - or, for a server known by its pin, a validity check against a clock that `timed` has not set yet is logged and not enforced. The pin still binds the key.
- **The evidence wanted:** in a QEMU run with the RTC set to 2000-01-01, the agent connects and its next command is not `command_expired`. EFF's `effector` suite takes that check once both parts are in.

**EFF's own S5 part that needs nobody** is 801-EFF-0028: the agent restarts itself after 5 min with no accepted answer, so a spent flow grant or a restarted `tls` or `parse` does not lose the server, and it never lands in `init`'s quarantine. It goes through the landing queue today.
