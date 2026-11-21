# KRN: ready — 550-NET-0090 and 0091, the station on 5 GHz and its rate

**Ready:** `claude/550-NET-0090-the-station-on-5-ghz` eef1cafd56

**For round 166.** The station's side of DRV's 213-DRV-0087 and of its Ryzen runs.

**550-NET-0090:**
- the network's 5 GHz access point goes before a 2.4 GHz one when its signal is −70 dBm or stronger;
- a rejoin on a listen-only (radar) channel waits for the network's beacon and sends nothing before it.

**550-NET-0091:** the rate is chosen by the airtime a delivered frame takes, from each try's outcome (`Station::sent` takes `attempts`).

docs/network/station.md is 0.5.

**Tested** in this session's container, on one x86 tree of the four Ryzen branches (801-NET-0089, 801-NET-0092, 550-NET-0090 with 0091, 213-KRN-0270; t-ryzen d60a69b95d):
- the `net` suite (4 CPUs, with `wlansim`), PASS, the Wi-Fi checks with it;
- the `normal` suite, PASS;
- host tests: `runtime` (47), `boot_abi_host`, `safe_start_host`, `netpolicy_host`, `wifi_host`, `wlan_auto_host`, `wifijoin_host`, `bcm_country_host`, and `dot11`'s own (23 in `station.rs`);
- the changed crates (`display`, `init`, `netstack`, `wlan`) built for aarch64.

The aarch64 suites are the gate's. All four are in `fast-test` (533f004f4a and after) for the maintainer's runs on the Ryzen.
