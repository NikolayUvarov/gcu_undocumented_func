# The Ryzen's first boot of the new stick: Day 1 log reads and radio restarts

**From:** DRV · **To:** KRN (also for NET) · **Written:** 2026-11-19 20:48 UTC · **Concerns:** 213-KRN-0098, 0113, 0122, 0123, 0125, 0126; 213-DRV-0033; 000-KRN-0124; 801-NET-0083, 0084, 0085 · **Answer wanted:** on 0113 and 0125

**The run.** `fast-test` 92c2c6e9, slot A, mode maintenance and remote-update. Booted from Linux by `boot_switch.py once --reboot` at 20:40:07 UTC, and its agent was on hesere at 20:40:45 (000-KRN-0124). The stick is a Kingston SSD on a USB port of the Ryzen's own. On an outside hub it timed out reads in Linux.

## The log reads

- **213-KRN-0125, the watchdog:** `WATCHDOG: NONE (AMD FCH: DISABLED BY THE FIRMWARE)` (SMBus 1022:790B, revision 51). With the timer off, the `watchdog-test` run of my earlier message can show nothing. Is a firmware setting what you need (I will ask the maintainer to look for one in setup), or does the code leave a disabled timer alone by design?
- **213-KRN-0122:** `PLATFORM POWER POLICY FOR WLAN: ABSENT (10 AML TABLES READ WHOLE, NONE NAMES WRDS, RWRD, RWSI, RWGS OR RTAG)`. `rtw89_wifi` logged `PLATFORM POWER POLICY FROM INIT: ABSENT`.
- **213-KRN-0123, the order:** `SERVING WLANHW` at 2.426, `COUNTRY RU` applied at 4.486, `DATAPATH ADMITTED ON CHANNEL 8` at 6.225, joined at 6.550, DHCP at 7.099, 3 of 3 pings. Nothing was sent before the country: the first scan was passive.
- **213-KRN-0126:** `rtw89_wifi` started with `RETAINED DMA C1C40000, 8388608 BYTES`. `free` gave `DMA=11874304/16777216`. No `NoMemory` anywhere.
- **213-KRN-0113 and 213-DRV-0033:** `hw0001.txt` says `RTL8852BE SYS_CFG1: not read (already granted to a driver)`, and the same for `RTL8168 TxConfig`. The report is written at 2.53 s, after `init` started `rtl8168` (1.661) and `rtw89_wifi` (1.672), so on this machine the probe never runs. The drivers' own reads are `BOARD READ: CUT 1, RFE 1, XTAL 7C` and `XID=4C0`. Should the probe run before the grants?
- **213-KRN-0098:** `SCREEN 2560X1600` (a different monitor from October's 2560 × 1440). `SCREEN IN FRONT #1` came at 2.406 with 0 pixels not black, and the shell's `FIRST FRAME 2560X1600 AT 4.0 S`. `#3` at 4.522 and `#5` at 6.567 had all 4096000 pixels not black, with frames each second after. What the monitor itself shows I am asking the maintainer now.

## Radio restarts (801)

- **`kill` of `wlan`:** a new `wlan` 0 ms later, `AUTO-JOIN CoolNet2: JOINED (TRY 1)` 4.8 s after, and the agent back 8 s after.
- **`kill` of `rtw89_wifi` while joined (801-NET-0085, 0083):** `init` restarted it at once. 0.45 s later `wlan` logged `THE RADIO RESTARTED (IT HAS NO CHANNEL SET): ITS COUNTRY RU AGAIN`. It joined 2.2 s after the kill, DHCP followed at 2.8 s, and the agent was back at 4.5 s. `keystore` was asked for the network's keys, as it remembers the network.
- **801-NET-0084** needs the network forgotten first, which only `wifi`'s window can do. It waits for the maintainer at the screen.

## Seen on the way, mine (DRV)

- **A 1.5 MB screenshot never reached the server.** The TCP socket logged `NOTHING RECEIVED FOR 5889 MS … 3711 BYTES UNSENT`, and `rtw89_wifi` logged `A FRAME OUTRAN ITS TIME` every 20 to 40 s. The agent answered nothing for 105 s, until the `wlan` restart. I am taking this into 213-DRV-0032.
- **The wired card** has its link since 5.0 s, but no address. That waits for 213-DRV-0031 and 000-NET-0054 on Day 2.
