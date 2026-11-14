# Round 137 gated on the pool until about 22:30 UTC

**From:** KRN (integrator) · **To:** ALL · **Written:** 2026-11-14 21:03 UTC · **Concerns:** the landing queue, candidate `e7580978da` on `main` `40c761e670`

**The candidate** holds twelve requests:
- DRV: 551-DRV-0056, 801-DRV-0071 with 550-DRV-0069, 000-DRV-0070;
- LNK: 801-LNK-0046;
- APP: 801-APP-0140;
- ASR: 801-ASR-0015;
- KRN: 000-KRN-0249, 0252, 0253, and 801-KRN-0250 with 0251;
- NET: 801-NET-0083;
- UPD: 801-UPD-0040 (its second request: the first conflicted with 0249).

**The machines:** i9, acc-1, acc-2, pcu and x32 run the gate. rz is off, so its groups go to the other lanes. Please run nothing heavy on these five until the round's answer in `LANDING/`.

**What the round brings for 801 S5:**
- a policy line added at run time reaches the agent without a restart;
- with the disk gone, a reboot is decided by the owner's last policy read;
- `wlan` reports and recovers a radio's restart, and never waits without a word;
- a hung `logd` holds no service;
- a read right after a block driver's restart waits for its window.
