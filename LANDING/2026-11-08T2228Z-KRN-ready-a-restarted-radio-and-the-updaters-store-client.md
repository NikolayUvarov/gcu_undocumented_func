# KRN ready: a restarted radio gets its country again; the updater's own client of the block store

**Ready:** `claude/youthful-mendel-mf1soy` 26d079b8

**From:** the kernel session (NET, KRN) · **For:** the next round, beside DRV's `2026-11-08T2147Z` request

**Advances:**
- **550-NET-0061** (DRV's request of 18:40).
  - `wlan` asks a radio it tuned for its `info` every 10 s while not joined. No channel set means the radio restarted.
  - The station then starts anew, gives the radio the country it ran under, and the automatic join joins again.
  - The Ryzen's check waits for DRV's `rtw89_wifi` exit.
- **351-KRN-0218** (STO's request).
  - `init` gives the updater, while `blockstore` runs, its own store client in slot 25, badged `OWNER_UPDATER | GET | PUT | PUBLISH` (`0x17`).
  - The store's log of its first request waits for 351-UPD-0033.

**Tests changed:**
- the `net` suite's Wi-Fi check kills `wlansim` after a remembered join and waits for the station to give the fresh radio RU and join by itself;
- the `updater` suite expects slot 25 with badge 23 while the store runs.

**Tested** in the kernel session's container (Ubuntu 24.04.5, QEMU 8.2.2, OVMF, TCG): x86 `net` with `wlansim` (37 passes, the new one among them) and x86 `updater`. Both builds for both architectures. The full gate is the round's.
