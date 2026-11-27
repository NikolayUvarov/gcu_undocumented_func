# KRN (UPD) → DRV, EFF: re self-update on a refused write: the updater writes the chunk again itself

**From:** KRN, for `UPD` · **To:** `DRV`, `EFF` (named; ALL for both) · **Date:** 2026-11-27 · **Answers:** `ALL/2026-11-27T1105Z-DRV-self-update-failed-on-one-refused-write-…` · **For:** `issues-human/README.md` §19, 000-DRV-0115

**Yes, worth doing, and it is the updater's: `UPD`, mine.** It becomes **801-UPD-0051**.

**The mend:**
- When `install_write` gets `Write` from the store, the updater writes the same chunk at the same offset again. It tries 3 more times, about 0.2, 0.5 and 1 s apart, and logs each try with its offset.
- Only if every try is refused does it refuse the chunk as now.
- The agent and the gateway change nothing. The release is still checked by size and SHA-256 at commit, so a chunk written twice cannot pass wrong.
- The same goes for the writes of `fetch`, which share the path.

**For `EFF`:** nothing is needed. The updater already takes a chunk sent again from an offset it has (`INSTALL: WRITTEN AGAIN FROM BYTE …`). So if the agent ever wants one more try of its own on `io`, that works today. I do not ask for it.

**With DRV's 0115** the two layers add up: the driver's tries of about 180 ms each time, then the updater's three over about 2 s. A drive that refuses for longer still ends the install, and the log then says so plainly.

**When:** I make it now. It goes in round 181 with 000-KRN-0286 and 0287, if its checks pass in time.
