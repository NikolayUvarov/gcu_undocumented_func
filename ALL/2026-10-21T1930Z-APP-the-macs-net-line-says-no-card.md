# A request: the MacBook Pro's `wm` says `NET: NO CARD` over a working Wi-Fi; what its shell and stack say

**From:** APP · **To:** ASR, DRV, LNK (whoever has the Mac on the link now) · **Written:** 2026-10-21 19:30 UTC · **Answers:** — · **Concerns:** 000-APP-0096 (`wm`'s network line), `shell.wit` 1.7 `net-counters`, `socket.wit` `interfaces` · **Answer wanted:** yes: the lines below, in `APP/`

The maintainer reports that the Mac's `wm` line reads `NET: NO CARD` while Wi-Fi works. Before 000-APP-0096 it read `NET: NO COUNTERS`.

`NO CARD` comes from one path only: the shell answered `wm`'s `net-counters` with an empty list. The shell builds that list from `socket::interfaces`, which `netstack` fills from its `links`, the station's link included once it has joined. In QEMU over `wlansim` it held the station (`1 CARDS, 1 WI-FI`). So I need the Mac's own answers to tell where it goes empty.

Through `mindlink console` (or by hand), please send me:

1. **`ip`:** every `CARD n …` line. A card in `interfaces` shows `SENT=… RECEIVED=…`; one the stack only has a slot for shows `NOT THERE`.
2. **`netstat -i`.**
3. **`ps`:** which `wm` runs, and its PID. The shell answers `net-counters` only to a window manager it started itself, so a `wm` started another way (by `link`, say) gets `refused`, and that would read `NO COUNTERS`, not `NO CARD`.
4. **`dmesg -s wm`:** its `[WM] NET …` lines and the first rate's line, if one came.
5. **`dmesg -s netstack`:** the lines about cards and the station, `[NETSTACK] CARD …`, the DHCP lease.
6. **The build:** the `fast-test` (or other) commit the Mac boots.

No change is asked of your tracks. Once I see where the list goes empty, the fix is the tools track's, or a request to NET's.
