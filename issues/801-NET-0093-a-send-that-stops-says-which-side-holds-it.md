# 801-NET-0093 — A TCP send that stops says which side holds it

**Type:** network (instrument) · **Owner:** `NET` (the kernel session), for DRV's Ryzen report · **Priority:** P1 · **Status:** in progress (landed in round 175; the Ryzen run is left) · **Blocked by:** — · **Main task:** 801 · **Constitution:** MC-12.1, MC-12.2

## Problem

DRV measured uploads from the Ryzen over its Wi-Fi to a PC on the LAN (`NET/2026-11-22T1955Z-DRV-a-tcp-upload-over-the-lan-goes-in-runs-of-20-segments-with-0-3-s-of-silence-between-and-one-stopped-for-nine-minutes.md` on `internal-dev-messages`).

**The upload runs a third of the time:**
- It goes in runs of about 20 segments, with silences of 0.3 to 0.5 s between them: 22.5 s of 33.5 s.
- There was one retransmission in the whole upload.

**One upload stopped for nine minutes:**
- There was no packet of that connection from the Ryzen: no retransmission and no probe, while the machine's other traffic went on.
- When its grant ended, the close sent the last 8,181 bytes `netstack` had taken: nearly its 8 KiB send buffer.
- DRV thinks the Effector agent's lost uploads in boot 82 were the same.

The record on the PC cannot tell apart the possible causes:
- segments `netstack` handed to `wlan` that `wlan` refused (each frame is one call; a refused frame is dropped and left to TCP's timer);
- smoltcp holding the data for a window or its own timers;
- the card's turn budget passing `netstack`'s visits over (`turn.rs`: 2000 ms of driver time in 5 s).

## Plan

1. **Count it (this step).**
   - **For each local TCP port,** `netstack` counts the segments it handed to a card, those with data, and those the card refused or did not answer. It also keeps the peer's last window field (before its scale) and when it came.
   - **A send that stops** is named once when bytes are queued and none is acknowledged for 5 s:
     `[NETSTACK] TCP SOCKET n TO peer: NOTHING ACKNOWLEDGED FOR … MS WITH … BYTES QUEUED (state): SINCE THE LAST, … SEGMENTS HANDED TO THE CARD, … WITH DATA, … OF THEM REFUSED; THE PEER'S LAST WINDOW FIELD … (… MS AGO)`
     It is named again when bytes are acknowledged: `… BYTES ACKNOWLEDGED AGAIN AFTER … MS`.
   - **Once a minute, a card** that refused frames or was passed over writes:
     `[NETSTACK] CARD i IN THE LAST MINUTE: … FRAMES TAKEN, … REFUSED OR UNANSWERED, … VISITS PASSED OVER AS ITS 2000 MS OF DRIVER TIME IN 5 S WERE SPENT`
2. **Mend what the counts name,** with DRV's bench run on the Ryzen.

## Acceptance criteria

- [x] `netstack` builds for x86 and aarch64; the budget's host cases (`tests/network_budget`) pass.
- [x] QEMU: an upload to a peer that stops reading is named, with window field 0, and again when the peer reads; the `net` and `netbench` suites pass.
  - `held_upload_check` in the `netbench` suite: a host reader that stops for 25 s after its first 64 KiB of an 8 MiB upload. The stop is named with 8192 bytes queued and the window field 0, then acknowledged again, and the upload completes.
  - QEMU x86, 4 CPUs, locally (2026-11-23), on a tree that also held 000-KRN-0276, 0277, 213-KRN-0278 and 801-NET-0094: `net` (with `wlansim`) and `netbench` passed.
- [ ] **On the Ryzen** (DRV, MC-12.1): a run of uploads shows the counts for the silences and for a stop if one comes.

## Progress

**2026-11-25:** landed in round 175 (main df11c5236d). DRV's bench run of uploads is left (asked in `DRV/2026-11-23T0536Z-NET-re-1955z-…`).

## Related

801-NET-0088 (a stream that stops while its client reads), 550-NET-0032 (the card's turn budget), DRV's `NET/2026-11-22T1800Z-…` (`wlan`'s wait between frames).
