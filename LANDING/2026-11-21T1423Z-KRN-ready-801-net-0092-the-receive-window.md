# KRN: ready — 801-NET-0092, the receive window at 128 KiB, and a download's summary

**Ready:** `claude/801-NET-0092-the-receive-window` a47981c3c2

**For round 166, after 801-NET-0089's request,** on whose branch it is.

**What it changes:**
- a client's connection receives into 128 KiB, with a scaled window;
- a connection past 1 MiB logs as it closes how fast it went and how often its client found the queue full;
- the `download` check adds up those lines;
- the stalled-stream check names the new capacity;
- docs/profile/network.md says the size.

**Tested** in this session's container, on one x86 tree of the four Ryzen branches (801-NET-0089, 801-NET-0092, 550-NET-0090 with 0091, 213-KRN-0270; t-ryzen d60a69b95d):
- the `net` suite (4 CPUs, with `wlansim`), PASS, the Wi-Fi checks with it;
- the `normal` suite, PASS;
- host tests: `runtime` (47), `boot_abi_host`, `safe_start_host`, `netpolicy_host`, `wifi_host`, `wlan_auto_host`, `wifijoin_host`, `bcm_country_host`, and `dot11`'s own (23 in `station.rs`);
- the changed crates (`display`, `init`, `netstack`, `wlan`) built for aarch64.

The aarch64 suites are the gate's. All four are in `fast-test` (533f004f4a and after) for the maintainer's runs on the Ryzen.
