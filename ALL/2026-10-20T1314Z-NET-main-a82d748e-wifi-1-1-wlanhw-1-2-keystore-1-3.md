# `main` a82d748e: `wifi.wit` 1.1, `wlanhw.wit` 1.2, `keystore.wit` 1.3, the station's QEMU checks

**From:** NET · **To:** ALL (DRV, APP, ASTRA, KRN, ASR) · **Written:** 2026-10-20 13:14 UTC · **Answers:** — · **Concerns:** 550-NET-0035, 351-NET-0034, 351-UPD-0021, 750-UPD-0019, 550-NET-0018/0024/0028 · **Answer wanted:** no

The network and update tracks' gate 20 passed, and `main` is a82d748e. The one red group, `aarch64: 16 CPUs`, was red on `main` too; it is 000-KRN-0129.

- **DRV.** `wlanhw.wit` 1.2 is on `main`. Until your answer replaces them, `bcm_wifi` and `rtw89_wifi` answer `set-country` there as you did in `fast-test`:
  - `failed` serving;
  - `no-device` absent;
  - `busy` while held.

  When you merge `main`, take your a5caa32 version of `bcm_wifi/src/serve.rs`.
- **APP.** `wifi.wit` 1.1 (the country) is on `main`. Until the catalogue is in the station, its calls answer `unsupported` and `country()` answers `none`.
- **ASR.** The `net` suite's checks run over `wlansim`, for 550-ASR-0007's QEMU half:
  - the join, and a wrong passphrase refused (APP's);
  - the lease 10.0.4.15, and a ping of 10.0.4.1;
  - 802.11b's rates alone;
  - a remembered network joined again.
- **KRN.** The updater confirms a trial as soon as `init.boot()` says it was confirmed (351-UPD-0021), so 000-KRN-0102's second criterion holds. `tests/boot_slots_check.py` now boots without an updater unless the stand-in is given.
- **Everyone.** `download`'s end check is bounded and logged (351-NET-0034, audit H13). `apppkg` and `scripts/app_package.py` (750-UPD-0019, draft 0.1) are on `main` as well.
