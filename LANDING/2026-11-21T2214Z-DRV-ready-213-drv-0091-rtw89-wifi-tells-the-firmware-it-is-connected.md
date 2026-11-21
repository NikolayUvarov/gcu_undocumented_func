# DRV landing request: `rtw89_wifi` tells the firmware that the station is connected (213-DRV-0091, step 1)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-21 22:14 UTC · **Concerns:** 213-DRV-0091 (new, advanced, stays open: step 2, the port, is not in this request)

**Ready:** `claude/213-DRV-0091-the-firmware-told-it-is-connected` e4fbbd891

**After** `LANDING/2026-11-21T2213Z-DRV-ready-213-drv-0090-…`: this branch contains 0090's head 96784295b, and so 0087's.

## What it carries

On `main` a9d6433db7 (merged in; `issues/README.md` resolved on the branch with both sides' rows).

- **What was wrong since the driver's first day.** The driver sent the firmware's join information once, as Linux sends it when an interface is added: disconnected, net type "no link". At association Linux sends it again as connected, net type infrastructure, and updates the address CAM with that net type and the association's AID (`rtw89_core_sta_link_assoc`, `rtw89_fw_h2c_join_info`, `rtw89_fw_h2c_cam`; the issue cites files and lines of the pinned source and decodes our words bit by bit). We never sent those. So the firmware and the MAC held the station to have no link.
- **The change:** when the data service sees its access point's association response (to our address, status 0), it sends the join information as connected and infrastructure, then the address CAM with infrastructure and the AID. On a new filter or a deauthentication it sends the no-link pair again. Port 0's registers are not touched (that is step 2, on its own branch, not yet on hardware).
- One log line at each: `THE FIRMWARE TOLD: CONNECTED TO <bssid> ON CHANNEL n AS AID m`.
- The issue file and its row. No ABI or IDL change.

## Tested

- **On LEG9:**
  - aeebd85d4 (this head before `main` was merged in): `02_build.sh`, then the full `scripts/host_tests.sh` with `MIND_SSHD`: 254 suites, 0 failed, exit 0;
  - e4fbbd891 (`main` merged in): all 40 `rtw89_*_host.rs` pass and the release build is clean. Its full host tests are queued; I add the result here when it comes.
- **On the Ryzen** (the same driver code, b3db40619; channel 8, country UZ chosen):
  - by the cable's `replace`, then as the boot image's driver since release 202611212016 (20:21 UTC);
  - **the access point's beacons reach the host: 581 a minute, against 0** with every driver before, in each boot;
  - a mirrored comparison of 6 MB downloads from the LAN shows **no difference in rate:** 246, 285, 265 KB/s with it; 300, 342, 223 with the driver before; 292, 259, 296 with it again;
  - the type filters and the port read the same before and after the two commands, so the beacons had been held back by the firmware's view of the link, not by a host-side filter;
  - no hole and no stall in the minutes measured.
- **Not claimed:**
  - that it cures the receiver's silences (213-DRV-0090) or the late sends: late sends turned out to follow the hour, not the driver, and no silence has been seen or provoked since; the night's log is read in the morning and reported;
  - anything about 5 GHz: with the beacons arriving, 213-DRV-0087's listen-only rule can now be met in data mode, but no 5 GHz join has been made.
- No QEMU suite runs this driver; the aarch64 groups do not build the crate.
