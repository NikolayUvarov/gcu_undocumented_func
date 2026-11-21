# DRV landing request: `rtw89_wifi` counts what it receives and logs each hole in the reception, with the registers (213-DRV-0090)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-21 22:13 UTC · **Concerns:** 213-DRV-0090 (new, advanced, stays open)

**Ready:** `claude/213-DRV-0090-the-receiver-goes-deaf` 96784295b

**After** `LANDING/2026-11-21T2212Z-DRV-ready-again-213-drv-0087-…`: this branch contains 0087's head ef2bb4f65. If you hold 0087 back for NET's fix, hold this one with it.

## What it carries

On `main` a9d6433db7 (merged in; the one conflict, `issues/README.md`, resolved on the branch with both sides' rows).

- **Why:** on a busy 2.4 GHz channel the Ryzen's receiver goes silent in both directions for 3 to 10 s; a 10 s silence makes the driver restart (801-DRV-0071), and one of those broke a release's download. The log could not tell a deaf receiver from quiet air.
- **Diagnostics only, in data mode; no change of behaviour:**
  - the joined access point's beacons are counted where the ring gives them;
  - **a hole** is 1 s without one of its beacons (3 s without any frame while no beacon has been counted): one line at its start and one at its end, with what arrived meanwhile and what was sent and acknowledged;
  - **a traffic line each minute:** frames, kept, filtered, invalid, inbox full, PPDU reports, the access point's beacons, sent, acknowledged, late, holes, the longest; a second line names the filter's BSSID and the beacons of others;
  - **a probe of about 60 registers,** all reads, after the first 12 holes' start and end lines, at the first minute and every tenth: MAC state, the receive ring's indexes, the type filters, the port, the coexistence grants, the baseband's gain and energy-detect fields, both RF paths' mode, channel and synthesizer lock;
  - the headers of the first 8 frames the station's filter refused, once a run.
- **What it costs:** one or two log lines a minute; the probe's reads at the moments named.
- The issue file and its row in `issues/README.md`. No ABI or IDL change.

## Tested

- **On LEG9:**
  - 1c83d581e (the branch before its last diagnostic commit): the full `scripts/host_tests.sh`, 254 suites, exit 0;
  - aeebd85d4 (the 0091 branch's head before `main` was merged in; it contains all of this branch's code): the full host tests, 254 suites, exit 0;
  - 96784295b: all 40 `rtw89_*_host.rs` pass (a new case: a missed beacon opens a hole and the next one closes it) and the release build is clean. The full host tests were not run on this merged head by itself.
- **On the Ryzen,** as the boot image's driver in seven boots of bench releases (19:09 to 20:38 UTC, with 30 MB downloads in five of them):
  - it showed that in data mode no beacon of any network reached the host (0 a minute, with 600 to 700 PPDU reports a second), which led to 213-DRV-0091;
  - the frame-based holes it logged at idle were pauses in the traffic, 1.0 to 1.5 s, which is why that fallback is now 3 s;
  - no probe read disturbed the link: the joins, the downloads and Effector's connection went on as with the driver before.
- **Not tested:**
  - a real hole logged with its registers: none has happened since the beacons count (213-DRV-0091's commit, about two hours of running when this was written); the night's log is read in the morning;
  - no QEMU suite runs this driver; the aarch64 groups do not build the crate.
