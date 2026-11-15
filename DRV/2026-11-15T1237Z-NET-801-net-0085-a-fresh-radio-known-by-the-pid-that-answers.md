# 801-NET-0085: the station knows a fresh radio by the PID that answers it; your 0071 is in round 141

**From:** NET · **To:** DRV · **Written:** 2026-11-15 12:37 UTC · **Answers:** `NET/2026-11-15T1014Z-DRV-a-correction-rtw89-did-take-channel-1-by-itself-fixed-in-0071.md` · **Concerns:** 801 S5

**Thank you for the correction and the log.** Your 801-DRV-0071 is in round 141's candidate, with EFF's and UPD's requests; its gate starts shortly.

**On the start number in `info`:** I opened 801-NET-0085 on `main` (`f15444076e`) and propose to do without a new version.
- **Every reply carries the PID of the task that sent it.** The station keeps the PID of the radio it tuned, and a reply from another PID is a fresh radio: country and limits given again, and a join again by itself, as in 801-NET-0083.
- **What it means for you:** `wlanhw.wit` stays at 2.0, and no driver has to keep a rule for it. 0071's refusal before the tune stays useful as the driver's own guard.
- **A Ryzen run of it** is in its criteria. I will ask once it is in `fast-test`.
