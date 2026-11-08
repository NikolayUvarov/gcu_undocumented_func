# 000-KRN-0217 — A safe start that leaves out only what it suspects, and brings it back by itself

**Type:** `init`, the run's mark (firmware variables) · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** track A · **Constitution:** MC-6.1, MC-6.3, MC-12.2, MC-12.3

## Problem

Asked by LNK for the maintainer (`KRN/2026-11-08T1513Z-LNK-a-safe-start-that-leaves-out-only-what-it-suspects.md` on `internal-dev-messages`). It refines 000-KRN-0076's steps 2–3.

**On the DEXP N100** (2026-11-08, build `d83c98c5`):
- **`boot0007.log`:** the run joined CoolNet2 and reached hesere. After 43 s the maintainer cut the power, before a minute past `READY`, so the run's mark stayed set.
- **`boot0008.log`:** a safe start. `bcm_wifi`, `video_gw`, `audio_gw`, `tts`, `rtw89_wifi`, `rtw88_wifi` and `bt_hci` were not started, and `wlan` was left out with them.
- **Its effect:** no network, so neither Effector nor the link could reach the machine; the screen alone said why.
- **Its cause was wrong:** every service had started and the network worked, so nothing pointed at a driver.
- **The watchdog** is `NONE` on this machine (the LPC bridge's ACPI I/O is off), so nothing told a freeze from a cut power.

**Why it happens.** `init` keeps one mark for the whole run (`RUN_MARK_NAME`) and clears it a minute past `READY`; the kernel clears it at every halt, restart and power-off asked of it. A run that ends before that minute otherwise, a cut power among them, is taken for a freeze, and the whole of `SAFE_LEFT_OUT` is left out.

## Plan

1. **A mark for each service of `SAFE_LEFT_OUT`.**
   - Set while it starts; cleared when it reports ready, or when 000-KRN-0075's limit ends its start. The run's own mark stays as it is.
   - Kept in one firmware variable (a bit per service), to bound the writes: one when a start begins and one when it ends.
2. **After a run that did not end as asked:**
   - **only the services whose own mark is still set are left out.** `bcm_wifi`, hung in its stage 2 on 2026-10-11, is that case;
   - **when every service was ready,** as on the DEXP, the boot is a normal one, with one line: the last run ended early, and nothing points at a service.
3. **A freeze is evidence; a cut power alone is not.**
   - **Evidence:** a run that the watchdog restarted (`IT RESTARTED THE LAST RUN`), or that ended in a panic. That leaves out what step 2 names at once.
   - **Without it,** a service is left out only after its start was cut short in two runs in a row: a count kept in the same variable.
4. **Back by itself.**
   - A safe start that left something out starts it again once the machine has run 5 minutes past `READY` without a fault.
   - One service at a time, each with its mark, and a line each, so that a freeze then names that one service at the next boot.
5. **Said where it can be read without a screen:** the safe start and what it left out, in Effector's telemetry and in the link's log. A machine with no network then says so the next time it has one, and a machine reached by the cable says so at once. Those are EFF's and LNK's parts, asked of them once the lines exist.

## Acceptance criteria

The `watchdog` suite's two boots exist already. Three cases in QEMU, with `system_reset` standing for a cut power:
- **a run cut after every service was ready** starts normally;
- **a run cut while a test service was still starting** leaves out only that one, after two such runs without evidence, or at once after the watchdog;
- **a left-out service comes back** after the stable time.

And: a power-button shutdown within a minute of `READY` is followed by no safe start (with 000-KRN-0216).

## Related

[000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md), 000-KRN-0075 (`init` waits for every service with a limit), 173-KRN-0036 (a safe start asked by a key), [000-KRN-0216](000-KRN-0216-the-power-button-shuts-down-cleanly.md).
