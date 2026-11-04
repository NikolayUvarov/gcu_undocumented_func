# Ready: the station's country, stage B (the beacons' country, else 00), and the tables' transmit power policy

**From:** KRN (also NET) · **To:** LANDING · **Written:** 2026-11-04 13:34 UTC · **Concerns:** 550-NET-0035 (stage B, advanced), 213-KRN-0122 (advanced)

**Ready:** `claude/youthful-mendel-mf1soy` e6112ebb8fe6e2675d3556c20bc246acc0035627

It contains `main` as of now.

## What it brings

- **550-NET-0035 stage B,** on the maintainer's decision of 2026-11-04: with no country chosen the station must work, not go quiet.
  - **`dot11::catalogue`** (on DRV's `regdb`) decides:
    - the user's choice, kept by `keystore`, first;
    - else the country most access points heard announce, from their Country element;
    - else `00`: send on 1 to 11, listen on 12 and 13.
  - **`wlan`:**
    - its first scan sends nothing;
    - it gives the radio each country's limits (`set-country`);
    - it logs what is applied and why.
  - **The interfaces:**
    - `wifi.wit` 1.2: the country calls are answered from the catalogue, and `country-origin` is new;
    - `wlanhw.wit` 1.3: `set-country` takes `00`.
  - **`wlansim`:**
    - it announces `UZ`, or nothing with the new test-only feature `no-country`, built by `scripts/x86_fixtures.sh`;
    - it logs a probe request that comes before any country.
- **213-KRN-0122:**
  - `mind::platform_power`: every DSDT and SSDT read whole and checked, and the five SAR names searched for;
  - `init` logs `PLATFORM POWER POLICY FOR WLAN: ABSENT …` or `UNKNOWN …`.

  It is not yet given to `rtw89_wifi`, which waits on DRV's argument parsing.

## Tested

- **x32-128 (KVM):**
  - `host tests` at 7549f1e5 (1159 s);
  - `x86: network, TLS and Effector` at 7549f1e5 (1034 s), every country check among them, `world_country_check` included;
  - `aarch64: network, TLS and Effector` at b7f2aea7 (954 s). Since then only the x86 check of the slow radio changed.

  Since 7549f1e5 only `issues/` and `main`'s text changed.
- **This container:**
  - `dot11`'s tests (19 station, 7 catalogue, 4 country element);
  - the policy's line on x86 and aarch64 QEMU.
- **What may move in other groups:**
  - every boot now logs the power policy;
  - `normal` requires its `ABSENT`;
  - the station's first scan only listens, so a join right after the radio's first answer waits for it.
