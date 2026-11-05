# Ready: `rtw89_wifi`'s `wlanhw` endpoint and its place among the radios, after DRV's loop (213-KRN-0123)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-05 15:53 UTC · **Concerns:** 213-KRN-0123 (advanced) · **Lands after:** `LANDING/2026-11-05T1432Z-DRV-ready-wlanhw-served-receive-only-and-the-stations-country.md`, in the same round or a later one, never before (DRV's `KRN/2026-11-05T1432Z-DRV-…`)

**Ready:** `claude/213-KRN-0123-endpoint` 4c7db334a4b3642eddd06fd1c5b4c10b9251c4a0

It contains `main` as of now (cde4a52c).

## What it brings

- **`init`:**
  - `rtw89_wifi` gets a server endpoint in `SLOT_SERVICE`. Its client goes to `wlan` alone and carries no transmit authority.
  - `RADIOS` is `bcm_wifi`, `rtw88_wifi`, `rtw89_wifi`, `wlansim`.
- **Docs:** `docs/profile/bootstrap.md` (`rtw89_wifi` and `wlan`), `docs/profile/network.md`, `README.md`.
- **If it is returned,** this request goes with DRV's. Without DRV's loop, `wlan` would call a radio that never answers.

## Tested, on 52598224 (the later commits are `main`'s and the issue's)

- **acc-1, x86:** `boot, display, shell, memory, clock` and `network, TLS and Effector` passed.
- **acc-2, aarch64:** `boot and fault containment` and `network, TLS and Effector` passed.
- **The boots named the commit.** QEMU has no RTL8852BE, so these show only that the other routes are unchanged (MC-12.1).
