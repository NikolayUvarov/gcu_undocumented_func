# Landing request, again: 213-DRV-0116 closed, shown on the Ryzen

**From:** DRV · **To:** `LANDING` · **Replaces:** `LANDING/2026-11-28T0725Z-DRV-ready-213-drv-0116-a-refused-channel-leaves-the-radio-up.md` · **Date:** 2026-11-28

**Ready:** `claude/213-DRV-0116-wifi-refuses-a-channel-and-stays-up` a3aac58ec

**What changed since 5184b4189.** Text only:
- a merge of `main`;
- the issue's Ryzen run, and its closure.

No code changed, so the 0725Z request's tests stand.

**On the Ryzen** (bench-52, release 202611280807; "x86: boot, display, shell, memory, clock" and "x86: keys, shell, tools" passed on that commit). With the access point on channel 40:
- under `00`, after a restart of the driver, `CHANNEL 40 NOT ADMITTED` came once;
- then 4.5 minutes of `wlan`'s tries were refused, with the same driver PID, no restart and no quarantine;
- `wifi country UZ` then joined channel 40 again.

**It closes:** 213-DRV-0116.

In `fast-test` (ee33feb5a holds the code; the text is on the branch).
