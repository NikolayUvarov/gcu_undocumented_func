# 000-APP-0095 — `ping`: the number of requests, their interval and wait, a fast mode; the size and the payload next

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (on the tools branch, 2026-10-20; the gate left) · **Blocked by:** — for `-c`, `-i`, `-w` and `-f`; the network track for `-s` and `-p` ([requests-NET.md](requests-NET.md), "An echo request's size and payload") · **Main task:** — · **Roadmap:** track D, G · **Constitution:** MC-12.3

**Recorded by:** the tools track, 2026-10-20, at the maintainer's request ("ping utility lacks number of probes parameter and delay and size and payload and fast mode. Add them.").

## Problem

The shell's `ping <host>` sends three echo requests back to back, each waiting 1 s, and prints the replies when all three are done. It has no options:

- not how many requests;
- not the delay between them;
- not their size or payload;
- no fast mode.

Its last line gives no loss or times, and nothing stops a run.

## Plan

- **`ping [-c count] [-i seconds] [-w seconds] [-f] host`** in the shell (`shell/src/ping.rs` reads the line and keeps the tally; `shell/src/net.rs` sends):
  - `-c`: 1 to 1 000 000 requests, 3 by default; `0` sends them until Esc or Ctrl+C;
  - `-i`: from one request's start to the next one's, 0 to 60 s with decimals, 1 s by default;
  - `-w`: how long each request waits for its reply, 0.1 to 30 s (the stack's bounds), 1 s by default;
  - `-f`: the next request at once, a dot for each one lost, no line for each reply, 100 by default.
- **Each reply's line on the screen as it comes;** Esc or Ctrl+C stops any run.
- **The last line:** `PING: 3 SENT, 3 RECEIVED, 0% LOST, TIME 100/340/800 US (LEAST/MEAN/MOST)`, with `, STOPPED` after Esc. Its start stays `PING: N SENT, M RECEIVED`, which the suites read.
- **A refusal that lasts ends the run:** `Denied` from the policy, `Invalid`.
- **`-s` and `-p`** say they wait for the network stack: `socket.wit` sends 8 bytes, `MINDCORE`. A call with a size and a pattern is asked of the network track (requests-NET.md).
- **Not in `console`:** its own `ping` answers when it is done, so a long run there needs `console` to print as it goes; it keeps its three requests.

## Acceptance criteria

- **Host tests** (`tests/shell_host.rs`):
  - the options and their bounds, `-f`'s defaults, decimals of a second;
  - the tally's line with and without replies, and after Esc.
- **The `net` suite** (x86, aarch64):
  - `ping -c 3 -i 0.5 10.0.2.2`: three replies, a second or more in all;
  - `ping -f -c 50 10.0.2.2`: 50 received and no reply lines;
  - `ping -f -c 4 -w 0.2 192.0.2.1` (TEST-NET-1, which answers no one): four dots and `100% LOST`;
  - `-s`, and a wrong `-c`, refused with their lines;
  - `ping -c 0` stopped by Ctrl+C: `^C`, then the tally with `, STOPPED`.
- **Later, with the network track's call:** `-s 1000` and `-p` to the gateway.

## Progress (2026-10-20)

- `shell/src/ping.rs` (`parse`, `Tally`) and `shell/src/net.rs` (`ping`, which renders the screen and watches Esc and Ctrl+C between requests through `msh::interrupted`).
- The help line; `docs/tools` §4.8 (EN, RU), with `ip` and `netstat`.
- Host tests pass (`tests/shell_host.rs`, 13). The `net` suite's `ping_options_check` is written.
- **Left:** the gate; `-s` and `-p` once the stack sends them.

## Related

[requests-NET.md](requests-NET.md) (the size and payload), [000-APP-0031](../issues-done/000-APP-0031-netstat.done) (`netstat`).
