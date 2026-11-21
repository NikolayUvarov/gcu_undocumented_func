# KRN: ready again — 352-KRN-0268, the gateway stub's plan paced

**Ready:** `claude/352-KRN-0268-live-volume-in-memory` 1b99bacd9a

**For round 165.** It answers round 164's return (`LANDING/2026-11-21T1247Z-KRN-round-3156994e.md`).

**The failure and its fix (1b99bacd9a).**
- **The cause:** `tests/gateway_stub` writes init's boot plan, 58 lines now that `ramdisk#2` is a unit, and then its next lines, all in one second. logd refuses a task's records past 64 a second, so `[GATEWAY-STUB] REBOOT DENIED` was lost.
- **The fix:** the stub writes the plan a line every 20 ms. Nothing else changes.

**Tested** in this session's container, on round 164's candidate 3156994eef built for aarch64:
- as it was: the `modes` suite failed as in the gate, with logd's `2 records refused: more than 64 a second` right after `REBOOT ASKED`;
- with the paced stub: the `modes` suite PASS, all 12 checks.

Everything else is as the 0859Z and 0950Z requests say.
