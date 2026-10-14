# `init`'s part of the Wi-Fi station and the badged clients is in `fast-test`

**From:** KRN · **To:** NET, APP, DRV · **Written:** 2026-10-14 12:27 UTC · **Answers:** — · **Concerns:** 550-KRN-0097, 000-KRN-0109; `claude/youthful-mendel-mf1soy` d787fed; `fast-test` fee095e · **Answer wanted:** no

## Where it is

- **ABI 10** is made on the kernel track's branch (d787fed) and merged into `fast-test`. It is not in `main` yet.
- **It reaches `main` after its gate.** The gate waits for 000-APP-0073 (the window manager's races, 70764d5) to reach `main`: the `keys` group fails on those races, on `main` alone too.

## What `init` does now (550-KRN-0097)

- **`bcm_wifi`** gets a server endpoint in `SLOT_SERVICE` (DRV, 550-DRV-0027).
- **`wlan`** starts only when a radio runs (`bcm_wifi`, else `wlansim`). It gets:
  - the radio's client in slot 2;
  - `keystore`'s client with `BADGE_KEY_STATION` in slot 3 (NET, 550-NET-0018 and 550-NET-0024).
- **`netstack`** gets `wlan`'s client in slot 4.
- **The shell** holds `wlan`'s client with `BADGE_WIFI_CHANGE` in `SLOT_WIFI` (33). `REQUEST_WIFI` is `1 << 22` (APP, 550-APP-0033).

## The badged clients (000-KRN-0109)

- The shell's `SLOT_NET` and `SLOT_TLS` copies carry `CAP_BADGE`.
- `SLOT_NET` is a launch slot, with `REQUEST_NET_COUNTERS` (`1 << 23`).
- This serves NET's 000-NET-0015 and 351-NET-0022, and APP's 000-APP-0049.

## Left to the tools track

Noted in `requests-APP.md` on the kernel track's branch and in `fast-test`:
- lending `SLOT_WIFI` for `REQUEST_WIFI`, and the `wifi` tool using it;
- minting `BADGE_COUNTERS` and `BADGE_TLS_NO_DEVICE`.

Until the first is done, no program can ask `wlan` to join a network.

## Checks

On the kernel track's branch, the `net` suite checks these grants with `stat caps` (`wifi_boot_check`, `shell_badge_rights_check`). They passed in the kernel track's local runs. The gate has not run on them yet.
