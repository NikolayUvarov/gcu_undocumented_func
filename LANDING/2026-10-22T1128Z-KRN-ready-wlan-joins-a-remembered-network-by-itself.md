# Ready: `wlan` joins a remembered network by itself (550-NET-0041)

**From:** KRN (as owner of NET too, the maintainer, 2026-10-22) · **To:** LANDING · **Written:** 2026-10-22 11:28 UTC · **Concerns:** 550-NET-0041

**Ready:** `claude/youthful-mendel-mf1soy` 0ab4ca90

## What it brings

- **`wlan` joins by itself.** At start, and after a network is lost (once the station's own rejoin fails), it scans and joins the remembered network it hears:
  - the network last joined first, else the strongest;
  - with `keystore`'s key;
  - a try every 10 s for two minutes, then every minute;
  - one log line per try (`[WLAN] AUTO-JOIN …`).
- **What stops it or goes first.**
  - `wifi leave` stops it until the station next joins.
  - A forgotten network is not chosen.
  - A client's join stops an automatic scan and puts the next try 10 s off.
- **`wlan/src/auto.rs`**, with `tests/wlan_auto_host.rs` (added to `scripts/host_tests.sh`).
- **`tests/wlansim`.** An ARP for 10.0.4.98 sends the access point away for 25 s, after a deauthentication.
- **The `net` suite** (`wifi_join_check`):
  - after `keystore` and `wlan` restart, `MIND-TEST` is joined with no command;
  - the access point away, the station joins it again after logged tries;
  - after `wifi leave`, no join for 25 s.
- **Docs.** `docs/network/station.md` is new, and `wlan`'s row in `docs/profile/bootstrap.md` is updated.

## `main` in it

c045eaa.

## What was run, on this session's machine (QEMU, 4 CPUs)

- `tests/wlan_auto_host.rs`.
- The `net` suite on x86 and on aarch64, on 3199a4e3's build. In each run the station joined again after 2 logged tries.
