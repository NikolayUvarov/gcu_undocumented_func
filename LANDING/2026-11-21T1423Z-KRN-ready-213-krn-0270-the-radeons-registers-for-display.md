# KRN: ready — 213-KRN-0270, the Radeon's registers for `display` (handoff 38)

**Ready:** `claude/213-KRN-0270-the-radeons-registers-for-display` 15d1a9cb4a

**For round 166.** It is on 352-KRN-0269's branch, which is in round 165. If round 165 returns 0269, this waits for it.

**What it changes:**
- the boot image `display`, after `clipboard` (53 images, `BootInfo` 1224 bytes);
- `init` grants `1002:1638`'s BAR 5 with the command register as the firmware left it, logged before and after, and never stops the device;
- `display` is left out of a safe start, and `data/services.txt` may turn it off;
- its image holds KRN's stub, which reads no register; `display/` is DRV's from now on;
- in QEMU, `display` is not started (`NO DEVICE`).

**Tested** in this session's container, on one x86 tree of the four Ryzen branches (801-NET-0089, 801-NET-0092, 550-NET-0090 with 0091, 213-KRN-0270; t-ryzen d60a69b95d):
- the `net` suite (4 CPUs, with `wlansim`), PASS, the Wi-Fi checks with it;
- the `normal` suite, PASS;
- host tests: `runtime` (47), `boot_abi_host`, `safe_start_host`, `netpolicy_host`, `wifi_host`, `wlan_auto_host`, `wifijoin_host`, `bcm_country_host`, and `dot11`'s own (23 in `station.rs`);
- the changed crates (`display`, `init`, `netstack`, `wlan`) built for aarch64.

The aarch64 suites are the gate's. All four are in `fast-test` (533f004f4a and after) for the maintainer's runs on the Ryzen.
