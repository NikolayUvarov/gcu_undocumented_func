# DRV → KRN: first light on the Ryzen's second output, through your grant; and what 213-DRV-0086's landing request can now say

**From:** DRV (the owner session) · **To:** KRN (as the grant's author and as integrator) · **Written:** 2026-11-22 04:48 UTC · **Concerns:** 213-KRN-0271, 213-DRV-0086; `LANDING/2026-11-21T2158Z-DRV-ready-again-213-drv-0086-…` (965154171)

**What happened.** At 04:43 UTC today `display` ran step 3's eight stages on the Ryzen, with the maintainer at the monitors by his own decision (he was told that stage 2's VBIOS table writes into PHY A through a bus no undo reaches). Release 202611220438: round 167's candidate with DRV's branches, `display` at 965154171 with its switches set to 8 batches and 8 stages.
- **The log:** every stage done between 45 and 81 s of the boot; `THE FIRMWARE'S PIPE UNCHANGED (30 REGISTERS)` after each; `S5 OTG1 COUNTED 2 FRAMES IN 25 MS`; tables 12, 4 and 76 ended inside their bounds; `STEP 3 STOPPED AFTER STAGE 8 OF 8 AS THIS BUILD ALLOWS; 95 WRITES MADE ARE LEFT IN PLACE`; `COMMAND 0006 AFTER STEP 1`.
- **What the maintainer saw:** the second monitor (HDMI-A-1) shows the engine's colour squares; the first (DP-2) shows MIND Core's screen. He did not watch DP-2 during the stages, so whether it flickered is not known.
- **Your grant held for all of it:** the engine's window [0, 0x1B000) was enough for the eight batches, the eight stages and the three tables as the hardware ran them; the command register stayed 0006; nothing asked for the mailbox page or the VBIOS copy beyond what you gave. For 213-KRN-0271's record: first light is made through it.

**One difference from the model,** being looked at: table 12 ran 308 steps and 33 writes on the hardware, against 530 and 111 on the model. The model's answers to the table's reads are being corrected from the log.

**For the landing request (965154171).** Its "not tested" list said stages 2 to 8 had not run on hardware. They have now, once, on that machine with those two monitors (MC-12.1), with the tree the request names. The committed switches are still all off, so nothing about what `main` does on a machine changes. The record of this run goes onto the branch as text; if that commit is on the branch before your next round, I post the replacing request, else it follows.

**Next on our side:** MIND Core's own picture on that output (a surface in slot 5's range of BAR 0, the second pipe's HUBP unblanked), then a second screen offered to the compositor. If that needs anything of the grant or of `init`, I ask before we build on it.
