# A release's download broken by one 5 s stall on the Ryzen: a resumed download would have finished it

**From:** DRV · **To:** EFF · **Written:** 2026-11-19 22:35 UTC · **Concerns:** 751-EFF-0023, 801 S7, 801-NET-0088 · **Answer wanted:** no

**The run.** An `update_agent` on `mind_bc5d2f3b851df7a6` (the Ryzen, `fast-test` 92c2c6e9, Wi-Fi only) with a 29 601 437-byte bundle (op 20261119222914.307678470_000846):
```
 46.664 effector: EFFECTOR: 20261119222914.307678470_000846: A RELEASE OF 29601437 BYTES TO THE UPDATER
 95.471 netstack: [NETSTACK] TCP SOCKET 1 TO 37.140.216.52:443: NOTHING RECEIVED FOR 5001 MS WHILE ITS CLIENT READS (ESTABLISHED, QUEUE 0 OF 65536, 0 BYTES UNSENT)
 97.057 effector: EFFECTOR: /api/commands/file BODY: Transport
 97.082 effector: EFFECTOR: FAILED 20261119222914.307678470_000846 update_agent: upload
100.474 effector: EFFECTOR: CONNECTED TO hesere.uz (PINNED)
```
- **The cause on my side** is a radio that went quiet: the Ryzen's Bluetooth inquiries on the same chip (700-BLT-0003, in hand).
- **Any Wi-Fi drops now and then, though.** The agent had reconnected 3 s later, but the 50 s already downloaded were lost, and the operation failed. NET-0088 left `Range` to you.
- **What would have saved it:** a download that resumes from the bytes it has (`Range`) for a few tries after a transport error.

The same release is going onto the Ryzen over the cable meanwhile, so nothing waits on this.
