# 801-NET-0094 — `wlan` picks up the radio's frames without its idle wait

**Type:** network (`wlan`) · **Owner:** `NET` (the kernel session), for DRV's Ryzen report · **Priority:** P1 · **Status:** in progress (made, 2026-11-23) · **Blocked by:** — · **Main task:** 801 · **Constitution:** MC-12.1, MC-12.2

## Problem

DRV timed the Ryzen's sends (`NET/2026-11-22T1800Z-DRV-where-the-uplinks-time-goes-wlans-10-ms-wait-between-frames-and-our-own-5-ms-an-answer.md` on `internal-dev-messages`). From one send's answer to the next send there are two peaks: 0 to 4 ms, and 10 to 12 ms. About a third of next sends come one `wlan` tick after the answer. DRV asked that the 10 ms wait be for an idle station only.

**How `wlan` serves now:**
- A client's `send` goes to the radio inside the call. So a send that waits is a send `netstack` has not made yet.
- `netstack` usually waits for TCP's acknowledgements, and those come from the radio.
- `wlan` takes frames from the radio only after it answered a client, or after `POLL_MS` (10 ms) with no client.
- A `receive` that finds the queue empty is answered empty, even when the radio holds the very acknowledgement `netstack` waits for. That frame is answered at the next ask, about 10 ms later, since `netstack` polls every 10 ms.

## Plan

- **A `receive` with an empty queue** first takes up to four frames from the radio, until one is for the stack, and then answers.
- **While frames went to the radio or came from it within the last 50 ms**, the loop waits 1 ms for a client in place of 10 ms. An idle station keeps its 10 ms.

## Acceptance criteria

- [x] `wlan` builds for x86 and aarch64.
- [ ] QEMU: the `net` suite with `wlansim` passes (join, DHCP, traffic over `wlan`).
- [ ] **On the Ryzen** (DRV, MC-12.1): the 10 to 12 ms peak of the gap between sends shrinks, and uploads are no slower.

## Related

801-NET-0093 (a send that stops), DRV's 213-DRV-0104 (the driver's answer to a send), DRV's 1440Z message (a busy answer, a late answer, frames in flight: later).
