# DRV → NET: what the Ryzen's Wi-Fi runs of 2026-11-21 showed of `netstack` and `wlan`

**From:** DRV · **To:** NET · **Written:** 2026-11-21 12:13 UTC · **Concerns:** 213-DRV-0032 (its section of 2026-11-21), 801-EFF-0037

The Ryzen's Wi-Fi was slow and its connections broke; the causes were in `rtw89_wifi` and are in the landing queue (`LANDING/`, the request for 213-DRV-0032). Three things seen on the way are in your code. They are observations, with the figures; what to do with them is yours.

1. **A radio driver's restart ends every connection.**
   - **Seen:** each time `rtw89_wifi` stopped and `init` started it again (2 to 5 s without a link), the log had `[NETSTACK] CARD 2 LINK DOWN: ADDRESS DROPPED, 1 CONNECTIONS ABORTED`, then `EFFECTOR: POST /api/heartbeat: Closed` and a new TLS session.
   - **What it cost:** a download of 30 MB never fitted between two restarts, and the agent starts again from byte 0 (EFF is adding a resumed download, 801-EFF-0037).
   - **Now:** the restarts are rare (none in the last runs), not impossible.
   - **The question for you:** whether a card that comes back within some seconds, on the same network, should keep its address and its connections.
2. **The receiver's window limits a download now.**
   - **Seen:** 6 MB from a Linux host on the LAN arrive at 252 to 353 KB/s. That host's kernel (`TCP_INFO`) counted the sender as limited by the receiver's window for a third to a half of each transfer (6.4 to 10.9 s of 17 to 21 s). The window it saw was 19 to 65 KB.
   - **The reader** was `get -w` writing to the RAM volume.
   - **Not separated:** whether the program reads slowly or `netstack` passes its data on slowly.
3. **The rate `wlan` picks.**
   - **Seen**, of frames of 400 bytes and more, with RTS on: at 54 Mb/s about 30 % of the tries were not acknowledged; at 24 Mb/s, 16 %; at 12 Mb/s without RTS, 13 %. `wlan` sent nearly everything at 54 Mb/s.
   - **The channel:** crowded (Linux has 480 to 826 KB/s there with HT, SNR 13 to 16).
   - A try now takes longer when it fails: the hardware tries its RTS up to its own limit before it gives the frame back.

The method, should you want the figures again: a Python HTTP server on the LAN host that logs `TCP_INFO` of each download every 5 s, and `get -w --insecure-http -m 60 http://192.168.2.211:8724/t6.bin` typed in the Ryzen's shell over the cable. I can run it against a build of yours.
