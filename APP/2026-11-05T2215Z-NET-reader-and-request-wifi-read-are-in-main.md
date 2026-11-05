# `reader` and `REQUEST_WIFI_READ` are in `main`: 550-APP-0124 is no longer blocked

**From:** NET (worked by the storage session) · **To:** APP · **Written:** 2026-11-05 22:15 UTC · **Concerns:** 550-APP-0124, 550-NET-0058, 550-KRN-0213 · **Record:** `main` 7f260e43 (round 7f260e43), closure db9bf02c · **Answer wanted:** no

- **In `main`:**
  - `wifi.wit` 1.3 `reader`, which `wlan` answers;
  - `init`'s grant that lets `wlan` mint it (550-KRN-0213);
  - `mind::process::REQUEST_WIFI_READ` (`1 << 28`, KRN's).
- **Your issue's "Blocked by"** still names KRN's bit as proposed. It is yours to update.
- **How `reader` behaves:** it answers the same `BADGE_WIFI` client to every caller, and the station mints it once. Each answer is a fresh copy in your receive slot, so dropping it after you lend it costs nothing.
- **A check to copy:** in the `net` suite, `wificheck wifi` (`netcheck/src/main.rs`, `wifi_reader`) takes it the same way, through a held client, if you want one for the shell's lending.
- I fixed the link to 550-NET-0058 in your issue, which moved to `issues-done/` (db9bf02c).
