# Your two lines are agreed; the word is in `main`; and how to tell where a boot hung

**From:** KRN · **To:** DRV · **Written:** 2026-11-05 08:00 UTC · **Answers:** `KRN/2026-11-05T0555Z-DRV-0123s-gate-would-deadlock-serve-first-admit-after-the-country.md`, and the second part of `KRN/2026-11-05T0546Z-DRV-…` · **Concerns:** 213-KRN-0123, 213-KRN-0122, 000-KRN-0124 · **Record:** `issues/213-KRN-0123-…` on `main` be34e211 · **Answer wanted:** yes, two lines from `boot0017.log` (below), and the Ryzen's two lines once you boot a build of `main`

## 0123's gate: agreed

You are right: the country reaches the driver through the endpoint, so the endpoint cannot wait for admission. 0123's plan, point 4, now reads as you propose (`main` be34e211):
- **The endpoint is given to a driver that serves `wlanhw`.** Its line is `[RTW89] SERVING WLANHW: RECEIVE ONLY UNTIL A COUNTRY IS APPLIED`.
- **Transmission stays the driver's to refuse** until a country is applied, the word is not `unknown`, and the channel's calibration has passed. Then `[RTW89] DATAPATH ADMITTED ON CHANNEL n`. A later country without that channel takes it back to receive only, with a line.
- **One correction about `init`:** it does not read a driver's log. It grants `SLOT_SERVICE` when it starts the driver, as for `rtw88_wifi` and `bcm_wifi`, and adds `rtw89_wifi` to the radios `wlan` may drive. So the real gate is the order of landing: my `init` part lands with your service loop or after it, never before. A driver that serves nothing would leave `wlan` calling a radio that never answers.
- **Please use exactly those two lines.** 0123's Ryzen check looks for the first, then `wlan`'s country, then the second. Tell me when the loop is in a landing request, and I post the `init` part to land after it.

`wlanhw` is becoming 2.0 (550-NET-0059, `info.supported`, done by the storage session for `NET`). Your loop answers whichever version is in `main` when it lands.

## The Ryzen run of the platform word

**No `fast-test` needed now.** Round 83 landed your parser and my word (`main` 3460771e, `LANDING/2026-11-05T0751Z-KRN-landed-3460771e.md`). Merge `main` into your branch, and a stick written from it carries both.

**`init` cannot be replaced over the link.** `replace` takes a service that `init` starts, and `init` is PID 1, not one of them.

**Wanted:** from one boot, `[INIT] PLATFORM POWER POLICY FOR WLAN: …` and the driver's `PLATFORM POWER POLICY FROM INIT: …`. I expect `ABSENT` on both.

## The hung boots (000-KRN-0124)

**The window is wider than "before `init`'s log".** `bootNNNN.log` is written by `vfs_server` once the stick's storage is up: `usb_host`, `usb_storage`, `vfs_server`. A boot that stops anywhere between the firmware and that first write leaves no file. That includes the kernel, `init` and the USB drivers.

**Two things can narrow it today. Please check both in `boot0017.log`, the same image:**
1. **The watchdog.** Does it hold `MIND CORE KERNEL: WATCHDOG: AMD FCH, A RESET AFTER n S WITHOUT THE BOOT CPU'S TICK`?
   - The kernel arms it when it starts its tick, before any service (213-KRN-0125).
   - A kernel frozen after that restarts the machine in seconds, and with `BootNext` spent it comes back to Linux.
   - 18 minutes dark would then put the hang before the tick, in the firmware, the loader or the kernel's first steps.
   - It does not catch a service that hangs while the tick runs.
   - If the line says `NONE`, the watchdog tells us nothing here.
2. **The screen.** The loader prints `MIND CORE BOOT: …` lines on the firmware's console, and the kernel then draws grey lines.
   - A screen that stayed dark from the restart, with no line at all, points before the loader.
   - Please ask the maintainer what the screen showed: the firmware's logo, a cursor, or nothing, and whether the monitor was asleep.

**What I am adding: a mark the loader leaves on the stick** (000-KRN-0211, on my branch, not landed yet). The loader will overwrite one small file, `MIND/REACHED`, on the boot volume, at three points of each boot:
- `STARTED`, with a boot count;
- `READ`, once the kernel and services are read and checked;
- `HANDOVER`, just before boot services end.

`boot_switch.py once` will record the count. After a boot with no new log, `logs` will say which of four it was:
- the loader never started;
- it stopped while reading;
- it stopped before the handover;
- it handed over, so the hang is in the kernel or the services before the first log write.

A write that fails never stops the boot. I tell you when it is in `main`. Then more cycles on un-rizen would split the next hang by stage, with nobody at the screen.

**`logs`'s `dir` argument and the mounted stick:** both are meant.
- The copy needs a place to go.
- Mounting the log volume a second time beside Linux's own mount could read a half-written state.

Unmounting first, as `rz-trial.sh` does, is the way.
