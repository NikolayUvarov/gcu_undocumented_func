# 550-APP-0117 — `wifi scan`: the networks around as text, without a screen

**Type:** tools (`wifi`) · **Owner:** tools track (`APP`) · **Priority:** P1 (the drivers track's hardware evidence) · **Status:** in progress (done in QEMU; DRV's runs on the DEXP and the MacBook Pro left) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track G · **Constitution:** MC-12.1

Numbered on 2026-10-30 from the drivers track's request (`APP/2026-10-30T0735Z-DRV-a-request-wifi-scan-without-a-screen.md` on `internal-dev-messages`), at the maintainer's word: "`wifi` needs a console variant of its network list".

## Problem

The list of networks exists only on `wifi`'s screen. A run over the cable or through Effector cannot read it:
- `effector485 run name=wifi args=scan` on the DEXP gave the usage;
- started without arguments, `wifi` printed only its `READY … MODE=LIST` line.

The drivers' hardware evidence needs the list as text: 550-DRV-0042 (the DEXP's RTL8821CE) and 550-DRV-0045 (the MacBook Pro's levels).

## Plan

- **`wifi scan`** beside `join`, `status` and `leave` (`wifi/src/headless.rs`):
  - it calls the station's `scan` (`idl/wifi.wit` 1.1);
  - it prints one line per access point, strongest first and an unmeasured level last: `[WIFI] NETWORK "MIND-TEST" 02:00:00:57:4C:A0 CHANNEL 6 -48 DBM WPA2 REMEMBERED JOINED`. The name is as `wifi join` takes it (quoted, with bytes that are not UTF-8 written `\xNN`), or `(hidden)`;
  - a last line gives the count: `[WIFI] SCAN: 15 NETWORKS`.
- **Exit statuses:** 0 even for an empty list, 1 when the station refuses, 3 without a station.
- **`USAGE`, `about!` and `docs/tools`** (EN, RU) name it.

## Acceptance criteria

- **Host test** (`tests/wifi_host.rs`):
  - the parse;
  - the demo station's ten access points, strongest first, the hidden one and a name that is not UTF-8;
  - the joined access point marked, its network's other access point not;
  - no station: 3 and its reason.
- **The `net` suite:** joined to wlansim's MIND-TEST, `wifi scan` marks it `JOINED` and ends with the count.
- **DRV's runs** on the DEXP and the MacBook Pro over the cable, recorded in DRV's issues.

## Related

[550-APP-0078](550-APP-0078-wifi-join-without-a-screen.md), [550-APP-0033](550-APP-0033-wifi-setup-program.md), 550-DRV-0042, 550-DRV-0045.
