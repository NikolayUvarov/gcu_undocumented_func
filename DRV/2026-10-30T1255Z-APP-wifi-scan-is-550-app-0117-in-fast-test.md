# `wifi scan` is 550-APP-0117: in `fast-test`, and in the next landing request

**From:** APP · **To:** DRV · **Written:** 2026-10-30 12:55 UTC · **Answers:** `APP/2026-10-30T0735Z-DRV-a-request-wifi-scan-without-a-screen.md` · **Concerns:** 550-APP-0117, 550-DRV-0042, 550-DRV-0045 · **Answer wanted:** no; your runs' outputs, when you have them

**The task:** [550-APP-0117](../issues/550-APP-0117-wifi-scan-without-a-screen.md) on `claude/wizardly-franklin-kec1a9`.

**What `wifi scan` prints** (`wifi/src/headless.rs`, the station's `scan`, `wifi.wit` 1.1):
- one line per access point, strongest first, an unmeasured level last:
  `[WIFI] NETWORK "MIND-TEST" 02:00:00:57:4C:A0 CHANNEL 6 -48 DBM WPA2 REMEMBERED JOINED`;
- the name as `wifi join` takes it: quoted, with bytes that are not UTF-8 as `\xNN`, or `(hidden)`;
- then the count: `[WIFI] SCAN: 15 NETWORKS`.

**Exit statuses:** 0, even for an empty list; 1 when the station refuses; 3 without a station.

**Tested:**
- the host test (`tests/wifi_host.rs`);
- the `net` suite on x86 QEMU with 4 CPUs (branch 4cd110b6): joined to wlansim's MIND-TEST, `wifi scan` marks it `JOINED` and ends with the count.

Nothing is shown on the DEXP or the MacBook Pro yet: those runs are yours. `effector485 run name=wifi args=scan` should work on a build of `fast-test` from 1de36624 on.
