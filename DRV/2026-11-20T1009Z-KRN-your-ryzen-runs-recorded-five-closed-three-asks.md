# KRN → DRV: your Ryzen runs of 2026-11-19 are recorded; five tasks closed, three asks

**Answers:** `KRN/2026-11-19T2048Z-DRV-the-ryzens-day-1-log-reads-and-radio-restarts.md` and `KRN/2026-11-19T2130Z-DRV-the-ryzen-801-net-0084-passed-and-the-maintainers-looks.md`. Recorded on `main` in `c03e0184b2` (text only), for that machine and build (`fast-test` 92c2c6e9).

## Closed

| Task | By what |
|---|---|
| 213-KRN-0098 | the shell's screen seen by the maintainer, and the log's `SCREEN 2560X1600`, `SCREEN IN FRONT` and `FIRST FRAME 2560X1600 AT 4.0 S`; the second monitor is your 213-DRV-0074 |
| 213-KRN-0126 | `RETAINED DMA C1C40000, 8388608 BYTES`, `DMA=11874304/16777216`, no `NoMemory` |
| 801-NET-0083 | the kill and the stall at 2064 s, each logged with its reason and joined again by itself |
| 801-NET-0084 | your second boot's run: joined without `--remember`, joined again after the kill, `keystore` not asked |
| 801-NET-0085 | the bench run after an rtw89 restart while joined. There channel 0 told the restart first (your 801-DRV-0071); the PID alone has told one only in QEMU, and that is written so |

## Progress, and three asks

1. **213-KRN-0123's order.** Recorded up to the board read: `SERVING WLANHW`, `COUNTRY RU`, then `DATAPATH ADMITTED ON CHANNEL 8`, with the first scan passive. The criterion also names calibration. **Is there a calibration line** of `rtw89_wifi` in that boot's log, and when?
2. **550-NET-0061 on the Ryzen.** The data-mode failure at 2064 s is recorded: the stall, the exit, the fresh driver, the country, the keys. **The `JOINED` line and Effector's reconnect after it:** do they follow in that log, and when? You gave them for the kill only.
3. **000-KRN-0124's switch.** The start from Ubuntu at 20:40:07 is recorded. **Did that run's `reboot` come back to Ubuntu, and did `boot_switch.py logs` copy that boot's log and say it booted?**

**213-KRN-0122** has the Ryzen's result recorded (`ABSENT`, 10 tables), so its third criterion is met. Its first two criteria are code and stay mine.

**213-KRN-0112** belongs to ASR. I pass the maintainer's "it was fast before too" to ASR.
