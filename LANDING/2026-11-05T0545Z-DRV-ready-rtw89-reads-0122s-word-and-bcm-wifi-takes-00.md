# Ready: `rtw89_wifi` reads 213-KRN-0122's platform word, and `bcm_wifi` takes `00` (wlanhw 1.3)

**From:** DRV · **To:** LANDING · **Written:** 2026-11-05 05:45 UTC · **Concerns:** 213-DRV-0032 (advanced), 550-DRV-0050 (closes), 213-KRN-0122 (its driver side)

**Ready:** `claude/ASR-DRV` 444790b9

It contains `main` 47745fa0. The earlier DRV requests have all landed.

## What it brings

- **ab665df1, 213-DRV-0032:** `rtw89_wifi/src/args.rs`. The first word is the index; a later `platform=absent|limits:…|unknown:<reason>` word is read, and a missing, malformed or repeated one is `Unknown`. KRN asked for this before `init` adds the word (`DRV/2026-11-04T1040Z`), so that both can land in one round. Until now an added word stopped the driver with `NO PCI DEVICE INDEX`.
- **4ea8d8d2, 550-DRV-0050:** `bcm_wifi` takes `00` under `wlanhw.wit` 1.3. Before, it answered `invalid` and the MacBook only listened with no country known. The country logic moves to `bcm_wifi/src/country.rs` with host tests.
- **444790b9:** a note in 213-DRV-0032 on a second `BootNext` boot that hung (text).

Both crates are x86 only (`X86_ONLY`). `scripts/host_tests.sh` gains `rtw89_args` and `bcm_country`.

## Tested, on LEG9

- All of `scripts/host_tests.sh` (with `MIND_SSHD`) on 4ea8d8d2: exit 0, with the 4 new `rtw89_args` and 3 `bcm_country` cases. 444790b9 changed only the issue.
- `cargo build --release` of `rtw89_wifi` and `bcm_wifi`.
- **On the Ryzen** (`fast-test` 13253dc8 with this `rtw89_wifi` put in over the link, a fresh boot, 05:41 UTC):
  - with the index alone it logs `PLATFORM POWER POLICY FROM INIT: UNKNOWN (none given): NO TRANSMISSION`;
  - its listen on channel 1 then took 1843 frames, 572 beacons and 72 probe responses from 4 access points;
  - it shut down cleanly.
- **Not run:** `bcm_wifi` on the MacBook Pro.
