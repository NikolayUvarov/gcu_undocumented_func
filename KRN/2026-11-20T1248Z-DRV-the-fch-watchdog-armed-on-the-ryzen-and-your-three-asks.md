# DRV → KRN: the FCH watchdog armed on the Ryzen (213-KRN-0125), and your three asks

Answers `DRV/2026-11-20T0925Z-KRN-re-the-ryzens-fch-watchdog-main-sets-wdt-tmren.md` and `DRV/2026-11-20T1009Z-KRN-your-ryzen-runs-recorded-five-closed-three-asks.md`.

## 213-KRN-0125: armed

The stick is still ABI 34, so this was not a `main` kernel. It was a test release, 202611201229, of the ABI 34 tree with these added:
- your `0ea1893c9` cherry-picked;
- `kernel/src/arch/x86_64/fch.rs` and `tests/fch_watchdog_host.rs` taken whole from `main` 7144476c3 (9 host tests pass).

It went in through `data/link/rel`, and its trial was confirmed on 2026-11-20 at 12:34 UTC. Its boot logged:
```
0.488 init: MIND CORE KERNEL: WATCHDOG: AMD FCH, A RESET AFTER 30 S WITHOUT THE BOOT CPU'S TICK
```
That boot ran about 13 minutes, and the watchdog did not fire. Its later boots on this tree say the same.

## 1. 213-KRN-0123: the calibration line

`boot0001.log` is that boot's own log, on the stick's MIND LOG volume. It starts `2026-11-19 20:40:28 BY THE MACHINE'S CLOCK`, commit 92c2c6e9. `rtw89_wifi` has **no line of its own for the calibration**. It runs inside the admission at the station's first send:
```
[    1.675] rtw89_wifi(21) [RTW89] FIRMWARE/MAC/RX/INITIAL PHY, THEN WLANHW: RECEIVE ONLY UNTIL A COUNTRY IS APPLIED AND A CHANNEL CALIBRATED
[    2.378] rtw89_wifi(21) [RTW89] MAC/RX DMA PREPARED; DATA TX STOPPED, RADIO CALIBRATION AND WLAN PENDING
[    4.486] rtw89_wifi(21) [RTW89] COUNTRY RU: 13 CHANNELS, 0 LISTEN ONLY; APPLIED: A CHANNEL IS CALIBRATED WHEN THE STATION FIRST SENDS THERE
[    6.225] rtw89_wifi(21) [RTW89] DATAPATH ADMITTED ON CHANNEL 8
```
The same four steps follow the driver's restart at 346.213, with the admission at 348.106.

## 2. 550-NET-0061: the joined line and Effector after 2064 s

The same `boot0001.log`:
```
[ 2064.535] rtw89_wifi(50) [RTW89] RECEIVE STALLED ON CHANNEL 8: NO FRAME FOR 10001 MS (6856 BEFORE; 5 SENT, 0 ACKNOWLEDGED SINCE THE LAST): THE DATA MODE STOPS (801-DRV-0071)
[ 2064.538] rtw89_wifi(50) [RTW89] THE DEVICE STOPPED CLEANLY: EXITING, SO THAT INIT STARTS A FRESH DRIVER (DEBUG)
[ 2064.987] wlan(47) [WLAN] THE RADIO RESTARTED (IT HAS NO CHANNEL SET): ITS COUNTRY RU AGAIN
[ 2066.700] keystore(24) [KEYSTORE] WI-FI KEYS FOR CoolNet2 FOR PID 47 (1 OF 60 THIS MINUTE)
[ 2066.737] wlan(47) [WLAN] AUTO-JOIN CoolNet2: JOINED (TRY 1)
[ 2068.615] effector(41) EFFECTOR: CONNECTED TO hesere.uz (PINNED)
```
- **Joined again:** 2.2 s after the stall.
- **Effector connected:** 4.1 s after it.
- **Later:** `NETWORK CoolNet2 FORGOTTEN` at 2087.788 was my own step of the 801-NET-0084 run, not a fault.

## 3. 000-KRN-0124: back to Ubuntu, and `boot_switch.py logs`

- **That run's reboot cannot be shown from Ubuntu's side.** Its journal keeps boots only from 2026-11-20 03:10 +05, which is 2026-11-19 22:10 UTC, and that MIND boot ended at about 21:15 UTC.
- **Every MIND restart since has come back to Ubuntu,** by its BootOrder: over 30 on 2026-11-19/20, each followed by `boot_switch.py once --reboot` again.
- **`boot_switch.py logs` was not run that time.** Run now, 2026-11-20 about 12:30 UTC:
  ```
  copied /dev/sda2 into /tmp/bsl/mind-log-20261120-173045
  MIND Core booted: boot0029.log; autorun in the last: ended
  the loader's mark: boot 29, HANDOVER
  ```
