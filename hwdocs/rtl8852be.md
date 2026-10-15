# RTL8852BE: source pins, firmware format and the first hardware observation

Status, 2026-10-15: `rtw89_wifi/` contains allocation-free format/register helpers, not a working driver. It does not power on the MAC, touch RF, start DMA, download firmware or associate. Tasks: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md), [213-DRV-0033](../issues/213-DRV-0033-rtl8852be-firmware-and-probe.md).

## Observed card

The maintainer's Ryzen `hw0005.txt`, build e6730d7a, identifies PCI `03:00.0`, `10ec:b852`, revision `00`, subsystem `10ec:b852`. BAR0 is I/O, base `0xE000`, size `0x100`; BAR2 is 64-bit MMIO, base `0xFCD00000`, size `0x100000`. PCI command is `0000`; memory decode and bus mastering are off. The legacy IRQ line is `255`, pin 1. These are observations from that boot, not constants to use as addresses in a driver.

## Pinned primary sources

Linux v6.19 is commit `05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`:

- [rtw8852b.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b.c): `RTW8852B_FW_FORMAT_MAX` is 1, firmware basename is `rtw89/rtw8852b_fw`, `try_ce_fw` is true; board/PHY/EFUSE dependencies live in `rtw8852b_chip_info`.
- [fw.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/fw.h), [fw.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/fw.c): `rtw89_mfw_hdr`, `rtw89_mfw_info`, `rtw89_mfw_recognize`, `rtw89_fw_hdr_parser_v0`, security/download handling.
- [reg.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/reg.h), [core.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/core.c), [core.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/core.h): register layout, `rtw89_read_chip_ver`, `rtw89_chip_info_setup`, firmware type IDs.
- [rtw8852b_common.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b_common.c): RFE, XTAL, country, TSSI, gain, thermal and PA-bias handling.

The listed source files carry `GPL-2.0 OR BSD-3-Clause`. The adapted public definitions use the BSD option, with [the retained notice](../LICENSES/BSD-3-Clause-rtw89.txt). No GPL-only implementation has been imported.

## Firmware pin and personal store

`linux-firmware` tag `20260221` resolves to commit `599764611a8ac213c6aa6dad17c941c2f46b53cb`. Use format **fw-1** with the source version above; merely selecting the newest `fw-2` would mix firmware and driver contracts.

| Item | Exact value |
|---|---|
| Binary | [rtw89/rtw8852b_fw-1.bin](https://gitlab.com/kernel-firmware/linux-firmware/-/raw/599764611a8ac213c6aa6dad17c941c2f46b53cb/rtw89/rtw8852b_fw-1.bin) |
| Size | 1,245,944 bytes |
| SHA-256 | `5b68415e3bfe72715d63a70703d4471b04b5475b8ff69cfdc8cdf48233cd5d3a` |
| Licence | [LICENCE.rtlwifi_firmware.txt](https://gitlab.com/kernel-firmware/linux-firmware/-/raw/599764611a8ac213c6aa6dad17c941c2f46b53cb/LICENCE.rtlwifi_firmware.txt), 2,115 bytes |
| Licence SHA-256 | `a61351665b4f264f6c631364f85b907d8f8f41f8b369533ef4021765f9f3b62e` |
| Store | `proprietary/firmware/rtw89/`, ignored by Git |
| Boot-volume destination | `data/firmware/rtw89/`, after writing the image |

Realtek permits unmodified binary redistribution with its notice and conditions; the licence prohibits reverse engineering, decompilation and disassembly. This work uses the published open-source header definitions, not disassembled firmware. The project's personal-store policy is still followed: neither binary nor licence download is packaged in an image or committed. The fetcher saves both together.

```bash
python3 scripts/firmware/rtl8852be.py fetch
python3 scripts/firmware/rtl8852be.py check
# After an image is written, explicitly copy the personal store to the intended volume:
scripts/proprietary.sh copy /path/to/mounted/MINDCORE
```

`fetch` uses an immutable source revision, a bounded read, exact size/hash checks and atomic replacement. `check` never uses the network. Neither command writes a disk. The existing `proprietary.sh copy` copies all files under `proprietary/firmware/`, retaining the Realtek licence alongside its binary; run it only against the intended written volume.

## Container and AX-v0 header

The public MFW format starts with signature `0xff` and an 8-bit image count in a 16-byte header. Each following 16-byte entry contains cut, firmware type, manufacturing flag, little-endian offset and size. Select the highest non-manufacturing cut not greater than the observed digital cut, with the desired type. Normal-CE is type 5; WoWLAN is 3; type 255 is log metadata. These identities are not interchangeable.

The pinned container has five entries:

| Cut | Type | Offset | Size |
|---|---:|---:|---:|
| 1 | 5 | 96 | 326616 |
| 2 | 5 | 326712 | 326536 |
| 1 | 3 | 653248 | 269896 |
| 2 | 3 | 923144 | 269824 |
| 0 | 255 | 1192968 | 52976 |

Both normal-CE images report version `0.29.29.15`, commit field `0x6fb3ec41`, AX header version 0, total header length 144 and three sections. There is **no compatible normal-CE entry for cut 0**. The Ryzen's Linux boot log establishes digital cut 1 (CBV), as recorded below; the selected normal-CE image is therefore cut 1, offset 96, length 326616. PCI revision alone does not supply the cut.

`firmware.rs` caps input at 4 MiB, images at 32 and sections at 10. It rejects image/table overlap, overlapping images, duplicate selection keys, malformed dynamic lengths, truncated sections, future header versions and trailing bytes. AX download addresses are decoded with mask `0x1fffffff`; section sizes include the optional eight-byte checksum trailer. The fixed AX transfer part limit is 2020 bytes.

Dynamic extension contents remain opaque after checking their enclosing length. Nonzero MSS signature pools, including formatted pools, are explicitly unsupported. Parsing a type-9 section with zero MSS count does not implement secure boot. No checksum contents, signature, download-address allowlist or RF compatibility is authenticated by this parser; the separate pinned SHA-256 check establishes the identity of the local file. A future downloader needs these policies before using the parsed addresses or data.

## Minimal read-only probe

Read one aligned 32-bit value at **BAR2 + `0x00F0`**, `R_AX_SYS_CFG1`; digital cut is `(value >> 12) & 0xf`. Linux `rtw89_chip_info_setup` calls `rtw89_read_chip_ver` before `rtw89_mac_pwr_on`, providing a primary-source basis for this initial read. Preserve the raw value. All-ones is an unavailable-device read; zero is suspicious/unconfirmed, even though the pure decoder extracts cut 0.

Validate the exact PCI ID and a usable MMIO BAR/window. The supplied run has memory decode off, so a KRN collector may need a scoped enable/restore of PCI command memory decode. Bus mastering and RF power are unnecessary for this read. Do not scan arbitrary offsets or enable DMA. No helper in `rtw89_wifi/` performs MMIO itself.

[213-KRN-0113](../issues/213-KRN-0113-realtek-boot-identification.md) adds this guarded read to the x86 hardware report: ungranted/non-mastering endpoint, D0 when PM exists, current BAR2 and actual uncached identity mapping. It temporarily enables MEM decode only when needed and verifies restoration without clearing PCI Status flags. Raw values and skip/failure reasons are retained. Eight host guard tests, an actual page-table-walk regression and the kernel build pass; the raw register observation by this MIND probe still needs a Ryzen run. Linux boot-log evidence of the digital cut is recorded below. Neither radio power nor an indirect analog-cut/EFUSE transaction is performed.

The subsequent analog-cut `XTAL_SI_CV` read in Linux uses an indirect transaction which **writes** request registers and polls; it is not part of this read-only slice. EFUSE access also requires reviewed power/indirect-access sequencing. No board calibration may be inferred from `SYS_CFG1`.

## Linux identity collector

On an existing Linux boot of the Ryzen, with its ordinary driver already bound:

```bash
python3 scripts/hardware/rtl8852be_probe.py --pci 0000:03:00.0 --output /tmp/rtl8852be-ryzen
```

Omit `--pci` to select the sole `10ec:b852` device, since enumeration can differ between operating systems. The output directory must be new. It is created with mode 0700, and `report.json` with mode 0600. The report contains kernel/architecture, PCI IDs and resource metadata, driver identity, interface administrative observations, and available firmware/RFE evidence. It does not mount debugfs, bind/unbind a driver, scan networks, configure a link, read PCI resource MMIO, reset or write the device. No SSIDs, keys, peer addresses, station lists or general kernel-log dump are collected.

The only external queries are `ethtool -i <interface>` and `journalctl -k --boot=0 --no-pager --output=cat --lines=32 --grep=<exact-device-numeric-identity-pattern>`. The journal filter admits only the driver's existing `chip rfe_type`, Linux 7.0 `chip info CID/CV/AID/ACV/RFE`, and `Firmware version` messages for the selected PCI function. Parsing independently enforces identity, numeric fields and their bounds; unrelated text is discarded. Conflicting chip records or RFE values leave the cut unresolved. The collector never turns on debug logging or firmware reports. Missing tools, journal access or matching messages are recorded; it does not escalate itself. Authorized administrative read access can be used for another run into a new output directory when journal access is denied.

An unambiguous chip-identity log and complete collection return **exit 0 (`identity-collected`)**; missing/conflicting evidence or query failures return exit 1 (`partial`). Exit 2 means a refusal/error, such as another device or an existing output directory. Boot-log evidence establishes the digital cut but not raw `SYS_CFG1`; the MIND probe's raw-register acceptance criterion in 213-DRV-0033 remains separate.

The source review at Linux v6.19 found the following boundaries:

- [`net/wireless/ethtool.c`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/net/wireless/ethtool.c), `cfg80211_get_drvinfo`, returns cached driver/kernel/firmware/bus strings. The firmware string may be `N/A`; the report preserves that limitation.
- [`net/mac80211/ethtool.c`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/net/mac80211/ethtool.c), `ieee80211_get_regs`, returns zero register bytes and a generic `wiphy->hw_version` field. No reviewed RTL8852BE mapping of that field to digital cut was established. The collector does not invoke `ethtool -d`.
- [`rtw89/debug.c`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/debug.c) exposes `btc_info`; [`coex.c`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/coex.c), `_show_cx_info`, prints `cv` and `rfe_type`. However, that **read handler also calls `rtw89_btc_fw_en_rpt`**, changing firmware reporting through H2C. The collector therefore does not read `btc_info`.
- The `read_reg` debugfs interface requires a selector write and uses mutable shared selection; `mac_reg_dump` reads whole register ranges. Neither belongs to this strictly read-only identity collector.
- Existing RFE and firmware boot messages originate in `__rtw8852bx_read_efuse` in `rtw8852b_common.c` and `rtw89_fw_update_ver` in `fw.c`. Reading already-emitted messages does not rerun their device initialization.

For the actual Ubuntu 7.0 boot, a further review of upstream Linux v7.0 commit `028ef9c96e96197026887c0f092424679298aae8` found [`core.c:6535`](https://github.com/torvalds/linux/blob/028ef9c96e96197026887c0f092424679298aae8/drivers/net/wireless/realtek/rtw89/core.c#L6535): `rtw89_chip_info_setup` emits `chip info CID: %x, CV: %x, AID: %x, ACV: %x, RFE: %d`. CV is the cached digital cut read from `SYS_CFG1`; the first four fields are hexadecimal, RFE decimal. This replaces the separate RFE message. The reviewed source explains the observed log format; it does not assert that Ubuntu's entire driver is identical to upstream.

The collector is an independent consumer of existing Linux interfaces; no Linux implementation is copied into it. Its fourteen host tests cover device/bus mismatch, ambiguity, invalid legacy/new identity messages, conflicting cuts/RFE, privacy filtering, absence of MMIO/debugfs access, missing tools/driver, timeout/permission failure and private output preservation. One test requires the installed `journalctl` executable and checks the production options against an empty temporary journal directory, without reading system logs. These checks are distinct from the Ryzen evidence below.

### Actual Ryzen Linux observation

At 2026-10-15 18:34:35 UTC, Effector operation `20261015183435.268000692_000025` on endpoint `un-rizen_2a8b108d5593` read the existing current-boot kernel record:

```text
rtw89_8852be 0000:03:00.0: chip info CID: 0, CV: 1, AID: 0, ACV: 1, RFE: 1
```

Thus digital cut is **1 (CBV)**, analog cut 1 and RFE 1. Preceding operations `20261015182720.825418577_000021` and `20261015182901.902681326_000022` identified kernel `7.0.0-38-generic`, bound driver `rtw89_8852be`, interface `wlp3s0`, D0/runtime active and carrier 1. The driver reported running firmware `0.29.29.5` (`da87cccd`), types 3 and 5, loaded from `rtw8852b_fw-1.bin`. That installed Ubuntu file differs from MIND's pinned personal-store firmware; it was not replaced.

No debugfs selector, raw MMIO, RF transaction, driver reload or network-state change was used for this observation. The private operation reports remain outside the repository. The pinned file's SHA-256 and explicit proprietary-fixture parser check were rerun successfully after identifying cut 1: its compatible normal-CE image is version `0.29.29.15`, offset 96, size 326616. No download or operation of that image on the Ryzen has been tested in MIND. The raw `SYS_CFG1` value remains uncollected.

## What blocks radio operation

`rtw8852b_chip_info` requires physical EFUSE 1216 bytes, logical EFUSE 2048 bytes (limit 1280), DAV physical/logical regions 96/16 bytes, and PHY capability data at `0x580`, size 128. The resulting board state supplies MAC address, RFE type, country, XTAL trim, TSSI, gain offset, thermal/power/PA calibration. RFE 1 is now observed through Linux; the remaining calibration values have not been collected and none is yet read by a MIND RTL8852BE driver.

Next implement the bounded RTL8852B MAC power state machine and board reads, then validate RFE-specific BB/gain/RF-path/NCTL tables and calibration. Firmware H2C/C2H transport, secure-boot handling, PCIe rings/interrupts, ownership recovery, channel control, RX/TX and the WLAN service integration are all still absent. Existing WPA2 logic can be reused only after the radio provides the necessary frame interface. No DHCP, ping or Effector claim follows from host parser tests.

## Reproducible host evidence

```bash
rustc --edition=2021 --test tests/rtw89_firmware_host.rs -o /tmp/rtw89_firmware_host
/tmp/rtw89_firmware_host
python3 tests/rtl8852be_fetch_test.py
python3 tests/rtl8852be_probe_test.py
rustc --edition=2021 --crate-type lib rtw89_wifi/src/lib.rs -o /tmp/librtw89_wifi.rlib
python3 scripts/firmware/rtl8852be.py check
RTW89_FIRMWARE="$PWD/proprietary/firmware/rtw89/rtw8852b_fw-1.bin" /tmp/rtw89_firmware_host --ignored
```

On 2026-10-15, 12 synthetic Rust tests, five offline fetcher tests, the `no_std` build and the separate two-cut proprietary-file validation passed. The normal suite intentionally does not require or embed the proprietary file. These checks do not exercise the Ryzen hardware or establish radio functionality.
