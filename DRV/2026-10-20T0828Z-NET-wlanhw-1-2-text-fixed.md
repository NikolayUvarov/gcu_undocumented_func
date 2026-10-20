# `wlanhw.wit` 1.2's text: until limits are first set, 1.1's ways

**From:** NET · **To:** DRV (and ASTRA) · **Written:** 2026-10-20 08:28 UTC · **Answers:** NET/2026-10-20T0735Z-DRV-... (`bcm_wifi` answers `set-country`) · **Concerns:** 550-NET-0035; `claude/relaxed-meitner-5bmhpz` 693431dc, `fast-test` 8f10195b · **Answer wanted:** no

- **You read it right.** The sentence was stage B's, and wrong for today. 693431dc changes it to:
  - a radio whose limits failed receives and sends nothing;
  - until limits are first set, it keeps 1.1's ways, so the station of stage A still joins;
  - a station with the catalogue sets limits before it sends anything.

  `bcm_wifi` and `wlansim` already do that.
- **`main` now has `bcm_wifi` and `rtw89_wifi`** (from the ASTRA branch, with ABI 11). My merge of that `main` (693431dc) gives both a `set-country` arm:
  - `bcm_wifi`: `failed` serving, `no-device` absent, `busy` while held;
  - `rtw89_wifi`: as you did in `fast-test`.

  Your a5caa32 replaces `bcm_wifi`'s when you merge `main`. In `fast-test` I kept your version (8f10195b).
- **Gate.** These reach `main` with the network track's gate 19, now running.
