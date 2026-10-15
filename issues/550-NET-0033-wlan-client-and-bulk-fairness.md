# 550-NET-0033 — Serve accepted WLAN clients before a bounded receive batch

**Type:** network reliability · **Owner:** ASTRA auditor-fixer, for `NET` · **Priority:** P1 · **Status:** in progress · **Blocked by:** integrated QEMU and slow-radio/hardware validation · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-5.2, MC-6.1, MC-12.1, MC-12.3

## Problem

The station accepts a client request, processes up to 64 radio receives with
one two-second deadline, and only then decodes and replies to the client.
The stack's fair turn in 550-NET-0032 can expire after 250 ms. An otherwise
healthy, busy radio can therefore delay an already accepted request beyond
the caller's timeout. This is a source finding, not an established cause of
the maintainer's MacBook packet loss.

## Plan

- Dispatch an accepted client before the background radio pump.
- Start at most eight background receive steps, checking a 100 ms cooperative
  slice before each step. Complete a started frame and its station actions.
- Keep the existing shared two-second deadline for radio/control/key calls;
  do not turn the cooperative slice into a shorter IPC timeout or discard
  an action that the station has already produced.
- Preserve authentication, association, handshake and link-loss timers. Use
  current uptime for received frames and for the timer tick after a batch.
- Test production ordering and batching with a client arriving during a
  continuous stream, a full frame/action crossing the slice, and an empty
  radio. Run the station's existing protocol tests unchanged.
- Build and commit on the own branch, publish to fast-test, then run focused
  tests on the own branch. Record integrated and hardware results separately.

## Acceptance criteria

- An accepted cached-info or queued-receive request is dispatched before any
  new background receive, including when the radio already has a long queue.
- With successful 10 ms radio receives, a client arriving during a continuous
  frame batch gets a dispatch opportunity after at most eight receives;
  time exhaustion prevents starting another frame after the 100 ms slice.
- A started radio/control/key operation retains the shared two-second
  deadline, and the current frame's generated actions are applied before
  yielding. No new short-deadline failure is reported as a dead radio.
- Production-helper regressions, existing network-budget tests and station
  protocol tests pass; integrated station/network QEMU checks pass.
- The report names the remaining limit: one slow synchronous radio/control/
  key operation, client handler or CPU work can exceed 100 ms and the stack's
  250 ms wait. This change is not a complete asynchronous station, a strict
  250 ms reply guarantee, or measured MacBook connectivity.

## Remaining limit

The cooperative limit is checked between frames. A request arriving during
one long synchronous call still waits. Preserving that call's two-second
contract while guaranteeing shorter client latency requires resumable or
asynchronous I/O, or an independently established radio latency contract.
Do not close this issue as a full WLAN latency guarantee.

## Progress (2026-10-15)

The implementation uses `wlan/src/service_turn.rs` for production dispatch
ordering and cooperative receives. `wlan/src/main.rs` retains one `Deadline`
per loop and checks it before starting the pump or the post-batch station
tick. The existing frame actions and protocol timer constants are unchanged.
The existing possibility of exhausting the aggregate deadline partway through
an action sequence remains; no new short IPC deadline is introduced.

On the own branch based on `a99cc44a650b2d4cf287116dfa7adb520277ca27`,
`cargo build --release --target x86_64-unknown-none` in `wlan/` passed in
5.65 s. The 26 network-budget host cases (18 existing, eight added) and the
unchanged `dot11` tests compiled with `cargo test --no-run`; they have not
run yet. Execution follows publication of this commit to `fast-test` and
occurs only on the own branch. Integrated QEMU and hardware evidence remain
open.

## Related

[550-NET-0030](550-NET-0030-the-stack-and-the-station-do-not-wait-for-ever.md),
[550-NET-0031](550-NET-0031-bounded-driver-calls-and-ingress.md),
[550-NET-0032](550-NET-0032-whole-stack-turn-budget.md),
[network profile](../docs/profile/network.md).
