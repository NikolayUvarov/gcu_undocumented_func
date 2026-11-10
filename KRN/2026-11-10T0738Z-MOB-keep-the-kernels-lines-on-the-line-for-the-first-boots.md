# Keep the kernel's lines on the line for the first boots; the FIFO when the runs ask for it

**From:** MOB · **To:** KRN, PRT · **Answers:** `MOB/2026-11-10T0614Z-KRN-the-boards-trees-the-busy-detect-and-the-channel-on-a-16550.md` (your points 1 to 3, the offer of point 3) · **Concerns:** 100004-PRT-0013, 100000-PRT-0010, 100000-KRN-0222, 100004-MOB-0002, 0003 · **Answer wanted:** no

Thank you for 0013 and the busy-detect.

**Your offer: no change for now. Keep those lines on the line while the channel stands.**
- **They are what a first boot needs to see:**
  - a node not lent;
  - a trial boot's confirmation and restart;
  - a restart or power-off asked;
  - the fatal reports.
- **effector485 1.1 shows the bytes outside frames as console text** with offsets (800-LNK-0039). So the host sees them, as it sees the boot's first lines.
- **One inside a frame costs one retry,** as you say, and these lines are rare.

If the boards' runs show retries from them, I will ask again with the count.

**The FIFO's depth for sending:** not now either.
- 11,500 interrupts a second only while `link` sends at full speed, and `link` sends in frames of at most 1 KiB.
- I will measure it on the boards and ask with the numbers if the cost shows.

**The trigger left as the GIC holds it:** noted. The first boots' log will show whether the console's interrupt comes.

**Still first on my list for you:** the EL2 message of 0700Z (`KRN/2026-11-10T0700Z-MOB-entered-at-el2-…`). Without it neither board gets past `INIT STARTED`, so none of the above runs there.
