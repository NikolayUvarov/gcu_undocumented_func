# For NET: the Ryzen's Wi-Fi alone, on channel 8 and on channel 100, and the wire's 30 minutes

**From:** DRV · **To:** KRN (`NET`) · **About:** 801-NET-0093, 0094, 0095, 550-NET-0096, 550-NET-0031, 550-NET-0062 · **Date:** 2026-11-26

All on the Ryzen, release 202611260856 (`netstack`, `wlan`, `netpolicy` are `main`'s of 6230e1fdb), boots 104 and 105; `netbench` to a server on the LAN (its side's timings kept). Each holds for that machine, that access point and that release (MC-12.1). Raw: DRV's `logs-2026-11-26/` (`net-ch8.txt`, `net-ch8-log.txt`, `benchsrv.log`, `wired30.txt`, `ping30.txt`).

**On channel 8, the Wi-Fi the only route (the wire out), 13:12 to 13:25 UTC.** One thing to weigh first: `bt_hci`'s stack was paging a speaker and trying an LE mouse the whole time (neither answered), on the radio the Wi-Fi shares. So these are lower than a quiet radio would give.
- **Down:** 4 MiB at 175, 57 and 148 KiB/s, 2 MiB at 166 (324 on 2026-11-22). `TCP SOCKET 2 CLOSED: 4096 KIB IN 27592 MS, 148 KIB/S; ITS QUEUE FULL AT 0 OF 1152 READS`.
- **Up (801-NET-0093):** 4 MiB at 26 and 24 KiB/s, 2 MiB at 23 (63 on 2026-11-22). The server's side of one: 2994 reads, the gap's median 8.3 ms, a tenth above 88 ms, the longest 2491 ms; its `TCP_INFO` rtt 49 ms.
- **The card's minute line, every minute of those runs:** `[NETSTACK] CARD 2 IN THE LAST MINUTE: 1202 FRAMES TAKEN, 74 REFUSED OR UNANSWERED, 827837 VISITS PASSED OVER AS ITS 2000 MS OF DRIVER TIME IN 5 S WERE SPENT` (1010 to 1423 taken, 72 to 94 refused, 785 to 828 thousand passed over). So the card's budget is spent all through an upload.
- **A stall seen:** `TCP SOCKET 1 TO 37.140.216.52:443: NOTHING RECEIVED FOR 5041 MS WHILE ITS CLIENT READS (ESTABLISHED, QUEUE 0 OF 131072, 725 BYTES UNSENT)`, `BYTES AGAIN AFTER 5699 MS`.
- **`wlan`'s drops (801-NET-0095):** `19 FRAMES THE RADIO HELD BACK AS BUSY DROPPED UNSENT IN ALL … PAST 1000 MS, OR BEHIND 16 WAITING`, in five lines over 13 minutes.
- **The driver's count for the same span:** `TRAFFIC ON 8: … 15352 SENT, 14875 ACKED, 426 LATE; 88 INBOX FULL`.

**On channel 100 under `00` (550-NET-0096), the Wi-Fi the only route, 12:37 to 13:10 UTC.** The join there went through today with two changes of DRV's (213-DRV-0087, requested for landing).
- **Down** 4 MiB at 229 and 306 KiB/s, **up** at 101 and 103 KiB/s, 500 of 500 round trips. In 33 minutes `TRAFFIC ON 100: … 18718 BEACONS; 14737 SENT, 14737 ACKED, 0 LATE; 0 HOLES`.
- **Two things of yours seen there:**
  - after `wifi country 00` (or back to `UZ`) the station scans and joins again, but the radio in data mode stays on its channel, so it finds only what is there: under `00` it joined channel 8 again; under `UZ` on channel 100 it said `AUTO-JOIN CoolNet2: Unsupported`. It moves only once the driver is restarted. Is a country change meant to make the radio scan anew? If so it is DRV's to make the driver leave data mode for it; say which.
  - with channel 100 failing (before the mend) the station tried it 14 times and then joined channel 8: good to know it does fall back.

**The wire (550-NET-0031's budget):** 30 minutes of 64 MiB transfers downward at a median of 4805 KiB/s with a ping a second beside them: 7 of 1800 lost, the median 746 us, a tenth above 179 ms, the longest 823 ms; and every minute `CARD 0 IN THE LAST MINUTE: about 65000 FRAMES TAKEN, 0 to 4 REFUSED OR UNANSWERED, about 337000 VISITS PASSED OVER AS ITS 2000 MS OF DRIVER TIME IN 5 S WERE SPENT`. So the same budget bounds a wired gigabit card at about 5 MiB/s and makes a ping wait up to 0.8 s. Is 2000 ms in 5 s meant for a card whose driver answers in microseconds?

**550-NET-0062, not tried yet:** the maintainer asked today that routes be switched by software, not by pulling the wire. I had asked him to pull it without reading your `card 0 off` and `default card 2` lines: my miss. Both are tried as soon as the wire is back in.
