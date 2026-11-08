# LNK to KRN: a safe start that leaves out only what it suspects, and returns by itself (000-KRN-0076)

**From:** LNK (development on real hardware), for the maintainer · **To:** KRN (`init`'s safe start) · **Follows:** `KRN/2026-11-08T1346Z-LNK-the-power-button-shuts-down-cleanly.md`

## What happened (the DEXP N100, 2026-11-08, build `d83c98c5`)

- **`boot0007.log`.** The run joined CoolNet2 (3 pings of 3) and reached hesere. After 43 s the maintainer cut the power. A minute past `READY` had not passed, so the run's mark stayed set.
- **`boot0008.log`.** The next boot was a safe start (`NOT STARTED: bcm_wifi, video_gw, audio_gw, tts, rtw89_wifi, rtw88_wifi, bt_hci`; `wlan` left out too).
  - **Its effect:** no network, so neither Effector nor the link could reach the machine, and the screen was the only place that said why.
  - **The cause was wrong:** nothing pointed at a driver, since every service had started and the network worked.
- **The watchdog** is `NONE (THE LPC BRIDGE'S ACPI I/O IS OFF)` on this machine, so nothing told a freeze from a cut power.

**The maintainer:** this cut-off of the network is confusing; make the safe start smarter.

## Asked (yours to number; 000-KRN-0076's steps 2–3 refined)

1. **Leave out only what was not ready.**
   - **The marks.** `init` marks each service of the safe start's list while it starts, and clears that service's mark when it reports ready (or 000-KRN-0075's limit ends its start). The run's own mark stays as it is.
   - **After a run that did not end as asked,** only the services whose own mark is still set are left out. `bcm_wifi`, hung in its stage 2 on 2026-10-11, is that case.
   - **When every service was ready,** as on the DEXP, the boot is a normal one, with one line that the last run ended early and nothing points at a service.
2. **A freeze is evidence; a cut power alone is not.**
   - A run the watchdog says it restarted (`IT RESTARTED THE LAST RUN`), or one that ended in a panic, leaves out what step 1 names at once.
   - Without such evidence, a service is left out only after its start was cut short in two runs in a row.
3. **Back by itself.** A safe start that left something out starts it again after the machine has run, say, 5 minutes past `READY` without a fault. The services start one at a time, each with its mark, and a line each. A freeze then names that one service at the next boot.
4. **Said where it can be read without a screen:**
   - the safe start and what it left out, in Effector's telemetry and in the link's log;
   - so that a machine with no network still says so the next time it has one, and a machine reached by the cable says so at once.

## How it can be tested

The `watchdog` suite's two boots already exist. Three more cases:
- a run cut after every service was ready starts normally;
- a run cut while a test service was still starting leaves out only that one;
- a left-out service comes back after the stable time.

QEMU's `system_reset` stands for a cut power.

Related: [000-KRN-0076](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md), 000-KRN-0075 (init waits for every service with a limit), 173-KRN-0036 (a safe start on a key).
