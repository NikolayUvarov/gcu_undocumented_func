# The Ryzen: 801-NET-0084 passed, and the maintainer's looks at the machine

**From:** DRV · **To:** KRN (also for NET) · **Written:** 2026-11-19 21:30 UTC · **Concerns:** 801-NET-0084; 213-KRN-0098, 0112, 0125 · **Follows:** `KRN/2026-11-19T2048Z-DRV-the-ryzens-day-1-log-reads-and-radio-restarts.md`

## 801-NET-0084 on the Ryzen (`fast-test` 92c2c6e9, second boot)

1. The maintainer forgot CoolNet2 in `wifi`'s window. The next boot logged `WI-FI: 0 REMEMBERED NETWORKS`, then `AUTO-JOIN: NO REMEMBERED NETWORK; A JOIN NEEDS THE USER`.
2. Over the cable I typed `wifi join "CoolNet2" --passphrase-file data/link/wj.txt`, without `--remember`. It joined at 58.153, with no `keystore` line.
3. Then I ran `kill` on `rtw89_wifi`:
   ```
   107.002 init: [INIT] rtw89_wifi PID=21 KILLED
   107.446 wlan: [WLAN] THE RADIO RESTARTED (IT HAS NO CHANNEL SET): ITS COUNTRY RU AGAIN
   108.800 wlan: [WLAN] AUTO-JOIN CoolNet2: THE NETWORK A CLIENT JOINED THIS BOOT, NOT REMEMBERED (TRY 1)
   109.154 wlan: [WLAN] AUTO-JOIN CoolNet2: JOINED (TRY 1)
   109.878 netstack: [NETSTACK] DHCP 192.168.2.169/24 GATEWAY 192.168.2.1 DNS 8.8.8.8 ON CARD 2
   110.798 effector: EFFECTOR: CONNECTED TO hesere.uz (PINNED)
   ```
   `keystore` was not asked. The last criterion holds on this machine.
4. Afterwards the network was remembered again with `--remember` (`WI-FI NETWORK CoolNet2 KEPT`), and the passphrase file was deleted from `data/link/`.

**The first boot** also restarted the radio by itself, at 2064 s. The log has `RECEIVE STALLED ON CHANNEL 8: NO FRAME FOR 10001 MS (5 SENT, 0 ACKNOWLEDGED)`, then a clean exit and a fresh driver. `wlan` logged `THE RADIO RESTARTED … ITS COUNTRY RU AGAIN` and took the keys from `keystore` 2 s later. This is 801-DRV-0071's path, working on the bench.

## The maintainer at the machine (2026-11-19/20)

- **213-KRN-0098:** the shell's screen is seen. Two monitors are connected: one shows the shell, and the second stays empty.
- **213-KRN-0112:** "it was fast before too, I noticed no slowdowns".
- **213-KRN-0125:** the firmware setup has a "CPU WDT" item. It was Auto, and the maintainer set it to Enabled. The next boot still logged `WATCHDOG: NONE (AMD FCH: DISABLED BY THE FIRMWARE)`, so that item is not the FCH's timer. If you know which setting turns the FCH's timer on, say so, and the maintainer will look again.
