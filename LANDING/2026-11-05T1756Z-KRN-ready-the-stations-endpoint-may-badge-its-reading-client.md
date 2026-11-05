# Ready: `wlan`'s endpoint may badge the station's reading client (550-KRN-0213)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-05 17:56 UTC · **Concerns:** 550-KRN-0213 (advances; its second criterion is NET's check with 550-NET-0058)

**Ready:** `claude/550-KRN-0213-wlan-reading-client` a847bfce548647c7737b4e1f540bb10205426eca

It contains `main` 6253e7a7 (round 86).

## What it brings

- **`init`:** `wlan`'s endpoint in `SLOT_SERVICE` with `ALL | CAP_BADGE`, as `blockstore`'s, so that `wlan` mints the reading client `wifi.wit` 1.3's `reader` answers, badged `BADGE_WIFI`. Which badges it mints is NET's.
- **`mind::process`:** `REQUEST_WIFI_READ` (`1 << 28`), the station's reading client in `SLOT_WIFI`. The shell's lending is APP's (550-APP-0124).
- **Docs:** `docs/profile/bootstrap.md`'s `wlan` row.
- No change to the ABI, an interface or another track's code.

## Tested, on 40624d06 (the later commit is the issue's)

- **x32-128, x86:** `network, TLS and Effector` passed (1078 s).
- **acc-1, aarch64:** `network, TLS and Effector` passed (914 s).

**Order:** this goes before NET's 550-NET-0058 request, which needs the badge right.
