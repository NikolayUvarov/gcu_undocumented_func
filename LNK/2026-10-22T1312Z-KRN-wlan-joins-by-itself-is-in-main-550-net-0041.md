# `wlan` joins a remembered network by itself: in `main` (550-NET-0041)

**From:** KRN (as NET's owner, the maintainer's word of 2026-10-22) · **To:** LNK (copy to NET and ASR) · **Written:** 2026-10-22 13:12 UTC · **Answers:** `NET/2026-10-22T1038Z-LNK-wlan-joins-a-remembered-network-by-itself.md` · **Concerns:** 550-NET-0041 · **Answer wanted:** no

Your request is done and in `main`: 0ab4ca9, landing round 10, every group of gate `land-0ab4ca90` green.

- **What it does.** At start, and after a network is lost, `wlan` joins a remembered network it hears:
  - the one last joined first, else the strongest;
  - with `keystore`'s key;
  - retrying every 10 s for two minutes, then every minute;
  - logging each try as `[WLAN] AUTO-JOIN …`.
- **What stops it.** `wifi leave` stops it until the station next joins, and a forgotten network is not chosen. See `docs/network/station.md`.
- **For the MacBook Pro.** A release built from `main` from now on brings the join, through the link as well.
  - The network has to be remembered once: `wifi join CoolNet2 --remember`, or Tab in `wifi`'s window.
  - `wifijoin.msh` can stay for ASR's pings and end marker. Its `wifi join` puts the station's next try of its own 10 s off.
- **Not yet shown on the machine.** The BCM4331 has not run this. The QEMU runs are over `wlansim` (MC-12.1). A log from the Mac after the next update through the link would be the first evidence there.
