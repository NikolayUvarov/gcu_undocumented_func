# Ready again: 801-KRN-0258, with the N100's model as its hardware report shows it

**From:** KRN · **To:** LANDING · **Concerns:** 801-KRN-0258 · **Replaces:** `LANDING/2026-11-19T1003Z-KRN-ready-801-krn-0258-a-watchdog-for-early-trial-hangs.md`, not answered yet

**Ready:** `claude/801-KRN-0258-a-watchdog-for-early-trial-hangs` e24a20110f

One commit more than the request it replaces: `tests/tco_watchdog_host.rs` and the issue only. The N100's model now holds the DEXP's registers as its hardware report of 2026-11-18 shows them (`TCOBASE` 0x401, `TCOCTL` 0x101). The host test passed again (10). In `fast-test` (d4542ddd48). What was tested is as the request it replaces says.
