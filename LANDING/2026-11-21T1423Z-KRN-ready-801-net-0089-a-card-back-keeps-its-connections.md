# KRN: ready — 801-NET-0089, a card back within seconds on its network keeps its lease and connections

**Ready:** `claude/801-NET-0089-a-card-back-keeps-its-connections` 683af93a89

**For round 166.** It answers DRV's Ryzen runs: a restart of `rtw89_wifi` aborted every connection.

**What it changes:** a link down with a lease holds the card's address, routes and TCP connections for 30 s. Back within the hold with the same lease, they go on. Otherwise they are aborted, as before.

**The `net` suite's two-card check:**
- a connection held through 3 s without the link, every byte reaching the host;
- down past 30 s, the connection aborted;
- DHCP's name check (000-NET-0050) asks for the held lease again.

**Tested** in this session's container, on one x86 tree of the four Ryzen branches (801-NET-0089, 801-NET-0092, 550-NET-0090 with 0091, 213-KRN-0270; t-ryzen d60a69b95d):
- the `net` suite (4 CPUs, with `wlansim`), PASS, the Wi-Fi checks with it;
- the `normal` suite, PASS;
- host tests: `runtime` (47), `boot_abi_host`, `safe_start_host`, `netpolicy_host`, `wifi_host`, `wlan_auto_host`, `wifijoin_host`, `bcm_country_host`, and `dot11`'s own (23 in `station.rs`);
- the changed crates (`display`, `init`, `netstack`, `wlan`) built for aarch64.

The aarch64 suites are the gate's. All four are in `fast-test` (533f004f4a and after) for the maintainer's runs on the Ryzen.
