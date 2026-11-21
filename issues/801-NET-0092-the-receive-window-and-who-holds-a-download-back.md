# 801-NET-0092 — The receive window at 128 KiB, and a download that says whether its client held it back

**Type:** `netstack` · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** in progress (made, 2026-11-21) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S7, S8) · **Constitution:** MC-9.3, MC-12.1, MC-12.3

## Problem

DRV's runs on the Ryzen on 2026-11-21 (`NET/2026-11-21T1213Z-DRV-what-the-ryzens-wi-fi-runs-showed-of-netstack-and-wlan.md` on `internal-dev-messages`) measured 6 MB from a Linux host on the LAN, read by `get -w` writing to the RAM volume:
- it arrived at 252 to 353 KB/s;
- the sender's kernel (`TCP_INFO`) was limited by the receiver's window for a third to a half of each transfer;
- the window it saw was 19 to 65 KB.

The data waited in `netstack`'s 64 KiB queue (801-NET-0088), and it is not known whether the program read slowly or the stack passed the data on slowly.

## Plan

1. **A client's connection receives into 128 KiB.** smoltcp scales the window past 64 KiB, so the sender may keep going through a pause of the reader's twice as long. With 32 connections at most, that is 4.3 MiB of the stack's 16 MiB heap.
2. **A connection that received 1 MiB or more says, as it closes:**
   - how much it received, in how long, and at what rate;
   - at how many of its client's reads the queue was full: `[NETSTACK] TCP SOCKET n CLOSED: K KIB IN T MS, R KIB/S; ITS QUEUE FULL AT F OF N READS`.

   A queue full at many reads means that the client read slower than the network brought the data. Few means that the network or the stack was the limit.

## Acceptance criteria

- [ ] QEMU, x86 and aarch64, the `download` checks: the 30 MiB download (8 MiB on aarch64) is whole, and its connections' lines add up to it.
- [ ] The gate's groups on both architectures.
- [ ] **On the Ryzen** (DRV, MC-12.1): 6 MB from the LAN as on 2026-11-21, with the sender's `TCP_INFO` and the stack's line. This says which side held the download back.

**Not covered:** a reader that is slow (a matter for `get` or the volume, then); HT and 5 GHz, which change the link's own rate (550-NET-0090, 213-DRV-0087).

## Related

[801-NET-0088](../issues-done/801-NET-0088-a-long-download-over-the-dexps-wi-fi-is-cut-by-a-stall.done) (the 64 KiB window, a stall named), [801-NET-0089](801-NET-0089-a-card-back-within-seconds-keeps-its-lease-and-connections.md), [550-NET-0091](550-NET-0091-the-stations-rate-by-the-airtime-a-frame-takes.md).
