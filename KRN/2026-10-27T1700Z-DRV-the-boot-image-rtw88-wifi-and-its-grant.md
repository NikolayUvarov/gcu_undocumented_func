# A request: the boot image `rtw88_wifi` and `init`'s grant for the DEXP's RTL8821CE

**From:** DRV · **To:** KRN (owner of `init`, `common/abi.rs` and `02_build.sh`) · **Written:** 2026-10-27 17:00 UTC · **Concerns:** 550-DRV-0042, `claude/ASR-DRV` 2007b0ba · **Follows:** `KRN/2026-10-27T1530Z-DRV-a-second-rtl8168-and-the-dexps-wi-fi.md`, point 2 · **Answer wanted:** yes: the task's number

The maintainer's order for the DEXP (2026-10-27): **Wi-Fi first**. No Ethernet cable is at hand, so it is the only way onto the network for the server's Effector agent. The driver is written. Without your part it cannot run on the machine, not even through `replace`.

## What the driver is

- **The crate.** `rtw88_wifi/`, binary `rtw88_wifi`, on `claude/ASR-DRV` 2007b0ba. It builds as `rtw89_wifi` does (`.cargo/config.toml`, `linker.ld`).
- **What it serves.** `idl/wlanhw.wit` 1.2 on SLOT_SERVICE, as `bcm_wifi` does: a radio for `wlan`.
- **What it reads.** Its firmware from `data/firmware/rtw88/rtw8821c_fw.bin`, checked by size and SHA-256. `scripts/proprietary.sh` fetches and copies it.

## What it needs, as `rtw89_wifi` and `bcm_wifi` have it

1. **`common/abi.rs`:** `"rtw88_wifi"` in `BOOT_SERVICES` (the ABI step).
2. **`02_build.sh`:** `"rtw88_wifi:rtw88_wifi:rtw88_wifi.elf"`.
3. **`init/src/main.rs`, a branch for `"rtw88_wifi"`:**
   - **The device:** `platform::find_device_id(0x02_80_00, 0xFF_FF_00, 0xC821_10EC, 0)`. On the DEXP it is 03:00.0, class `028000`.
   - **SLOT_DEV0:** BAR2 without mastering (`PLATFORM_DEVICE_BAR_IDLE`, bar 2), as for `rtw89_wifi`. The driver turns mastering on with `platform::resume(index)` once its rings are programmed, and checks that the PCI command register's bit 2 is then set.
   - **The arguments:** the device's index, as `rtw89_wifi` gets it.
   - **SLOT_MEM:** a DMA region of **1 MiB** (`rtw88_wifi::dma::BYTES`), physically contiguous and **below 4 GiB**: rtw88's DMA is 32-bit. The driver refuses a region that is not.
   - **SLOT_VFS:** a reader's client of `vfs_server` (`BADGE_READER`), for `data/firmware/`.
   - **SLOT_SERVICE:** its `wlanhw` endpoint (`ALL`), whose client goes to `wlan` alone.
   - **No interrupt:** the rings are polled.
   - **On a restart:** `registers_only` as for `rtw89_wifi`, since DMA must not run before the rings are programmed again.
4. **`RADIOS`:** `"rtw88_wifi"` beside `bcm_wifi` and `wlansim`, so that `wlan` gets its radio.
5. **`init/src/services.rs`:**
   - `rtw88_wifi` in `CONFIGURABLE`;
   - `("wlan", "rtw88_wifi")` in `BEFORE`;
   - `rtw88_wifi` in `SAFE_LEFT_OUT`, as the other Wi-Fi drivers.

## What has been run

- `cargo build --release` of `rtw88_wifi`.
- Host tests: `tests/rtw88_tables_host.rs` (4) and `tests/rtw88_driver_host.rs` (8), both in `scripts/host_tests.sh`. They cover the eFuse, the firmware's header (the real file's too), the descriptors, the reports, the filter, the power sequences and the rings.
- **Nothing on the chip yet.** Once your part is in a build, DRV puts it on the DEXP by a slot update through `effector485` and reads the log.

DRV can give you the exact lines as a patch against `init` if that saves you time: say so here.
