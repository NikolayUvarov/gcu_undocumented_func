# 801-NET-0095 — A radio that answers `busy` loses no frame and moves no rate

**Type:** network (`wlan`, `dot11`) · **Owner:** `NET` (the kernel session), for DRV's Ryzen report · **Priority:** P1 · **Status:** in progress (made, 2026-11-23) · **Blocked by:** — · **Main task:** 801 · **Constitution:** MC-6.2, MC-12.1, MC-12.2

## Problem

DRV found how a frame the RTL8852BE holds becomes a run of losses on a busy channel (`NET/2026-11-22T1440Z-DRV-on-a-busy-channel-one-held-frame-becomes-a-false-loss-a-slower-rate-and-dropped-frames-the-sends-answer.md` on `internal-dev-messages`).

**What the driver does:**
- `rtw89_wifi` takes one send at a time.
- While a frame waits in the MAC, often 90 to 220 ms, every other `send` is answered `busy`. `wlanhw.wit` says `busy` means "the transmit queue is full": nothing was tried on the air.

**What `wlan` does with `busy`:**
- It records the frame as one that failed 15 tries, and `dot11` steps the rate down for it.
- The frame itself is not offered again.

So one hold drops every frame offered meanwhile. Each drop steps the rate down, and TCP sees a run of losses. In boot 82 all four uploads of a log were lost.

## Plan (DRV's step 1)

- **`busy` is not a loss.** The frame, and every frame after it, waits in `wlan` in order. Up to 16 frames wait, each for up to 1 s.
  - Order matters: a frame sent ahead of a held one carries a later packet number, and the access point would drop the held one as a replay.
- **Offered again.** `wlan` offers the held frames again at its next turn, every millisecond while frames wait.
- **The rate.** A frame that goes later counts for the rate it was built with: `dot11`'s `Station::sent_rate`. A frame dropped unsent counts for no rate.
- **Drops are named.** A frame held past 1 s, or with 16 already waiting, is dropped. The log names those drops at most once a minute.
- **Later, not here:** DRV's steps 2 (a late answer says `pending`, a minor of `wlanhw.wit`) and 3 (a window of frames in flight).

## Acceptance criteria

- [x] `wlan` builds for x86 and aarch64.
- [x] `dot11`'s tests pass, with a case for an outcome counted at the rate the frame went at.
- [x] QEMU: `wlansim` built with `busy` answers every unicast data frame busy at its first offer.
  - Before this change, the join failed: the handshake's frames were dropped.
  - With it, the station joins, leases and answers every ping (`busy_radio_check`, the `net` suite).
- [ ] **On the Ryzen** (DRV, MC-12.1): on a busy afternoon, uploads of a log go through, and the rate does not step down at each hold.

## Related

801-NET-0094 (`wlan`'s idle wait), 801-NET-0093 (a send that stops), DRV's 213-DRV-0091 and 213-DRV-0102 (the holds and the detection floor).
