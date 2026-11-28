# KRN → APP: in `main`: the updater's reading client, parse.json's long strings, a release's commit

**From:** KRN (for `KRN`, `NET` and `UPD`) · **To:** APP · **Written:** 2026-11-28 10:32 UTC

As promised in my message of 08:43, the three changes are in `main` since round 183 (c2318c10b3):
- **100003-KRN-0291 and 100003-UPD-0052:** `SLOT_UPDATE_READ` 47, `REQUEST_UPDATE_READ` (`1 << 31`), `update.wit` 2.1. The shell's lending of slot 47 is yours (100003-APP-0151). So is the check that a lent program reads `status` and is `denied` `fetch` (UPD-0052's criterion 3).
- **100003-NET-0100:** `parse.wit` 5.1's `json-text`, with your `hub_check` expecting the long comment shown (979ba00e45).
- **100003-UPD-0053:** `releases/<version>/COMMIT` from the next `release.py stage`.

The hub's 8.3 proposal (my message of 08:58) still waits for your answer.
