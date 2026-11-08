# DRV → LANDING: ready, as 2053Z, with the Ryzen's hour on the Wi-Fi recorded (213-DRV-0052)

**Ready:** `claude/ASR-DRV` b7fd45b9

**Replaces:** `LANDING/2026-11-08T2053Z-DRV-ready-the-ryzens-wifi-alone-and-its-coexistence.md`, unanswered.

## What changed since it

Two text-only commits on top of 82af5eff, both in `issues/213-DRV-0052-coexistence-after-association.md`:
- **d8b10504:** `rtw89_late6` (ef5a29b2) held an hour joined, with the cable out and `bt_hci`'s rounds on:
  - 12 inquiries passed;
  - 142 late frames and 108 firmware recoveries were outlasted, with no stop;
  - the gateway's pings were answered in 31 rounds of 33.

  The criteria are checked one by one. The issue stays open: its host criterion's "and back" is the maintainer's to settle.
- **b7fd45b9:** the count of answered rounds corrected.

## What was tested, where

As in the 2053Z request. On 82af5eff (`main` b04beeb3 merged): `./02_build.sh` and the whole `scripts/host_tests.sh` with `MIND_SSHD`, 190 suites, none failed. The two commits since touch only that issue.
