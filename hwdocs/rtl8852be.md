# RTL8852BE: source pins, firmware format and the first hardware observation

Status, 2026-10-16: `rtw89_wifi/` has a native board-read cycle followed by one hash-pinned firmware diagnostic using retained command DMA, then independent shutdown. This is not a working network driver: RF calibration and the WLAN endpoint are absent. Native integration and actual hardware evidence are recorded separately below. Tasks: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md), [213-KRN-0114](../issues/213-KRN-0114-realtek-driver-startup.md), [213-DRV-0033](../issues/213-DRV-0033-rtl8852be-firmware-and-probe.md).

## Observed card

The maintainer's Ryzen `hw0005.txt`, build e6730d7a, identifies PCI `03:00.0`, `10ec:b852`, revision `00`, subsystem `10ec:b852`. BAR0 is I/O, base `0xE000`, size `0x100`; BAR2 is 64-bit MMIO, base `0xFCD00000`, size `0x100000`. PCI command is `0000`; memory decode and bus mastering are off. The legacy IRQ line is `255`, pin 1. These are observations from that boot, not constants to use as addresses in a driver.

## Pinned primary sources

Linux v6.19 is commit `05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`:

- [rtw8852b.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b.c): `RTW8852B_FW_FORMAT_MAX` is 1, firmware basename is `rtw89/rtw8852b_fw`, `try_ce_fw` is true; board/PHY/EFUSE dependencies live in `rtw8852b_chip_info`.
- [fw.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/fw.h), [fw.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/fw.c): `rtw89_mfw_hdr`, `rtw89_mfw_info`, `rtw89_mfw_recognize`, `rtw89_fw_hdr_parser_v0`, security/download handling.
- [reg.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/reg.h), [core.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/core.c), [core.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/core.h): register layout, `rtw89_read_chip_ver`, `rtw89_chip_info_setup`, firmware type IDs.
- [rtw8852b_common.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b_common.c): RFE, XTAL, country, TSSI, gain, thermal and PA-bias handling.

The listed source files carry `GPL-2.0 OR BSD-3-Clause`. The adapted public definitions and initial board-access sequences use the BSD option, with [the retained notice](../LICENSES/BSD-3-Clause-rtw89.txt). No GPL-only implementation has been imported.

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
scripts/proprietary.sh fetch  # both MacBook Broadcom and Ryzen Realtek firmware
# Realtek alone, without the Broadcom extractor dependency:
python3 scripts/firmware/rtl8852be.py fetch
python3 scripts/firmware/rtl8852be.py check
# After an image is written, explicitly copy the personal store to the intended volume:
scripts/proprietary.sh copy /path/to/mounted/MINDCORE
```

`build-and-write.sh` invokes `scripts/proprietary.sh fetch`, which now includes the pinned Realtek firmware and licence. They remain in the personal store until the existing post-write copy; neither enters the image.

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

Validate the exact PCI ID and a usable MMIO BAR/window. The supplied run has memory decode off, so a KRN collector may need a scoped enable/restore of PCI command memory decode. Bus mastering and RF power are unnecessary for this read. Do not scan arbitrary offsets or enable DMA. This probe is independent of the later native board-access service described below.

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

## Initial native board access (213-DRV-0032)

The new `rtw89_wifi` executable consumes the exact RTL8852BE MEM-only BAR2
contract from [213-KRN-0114](../issues/213-KRN-0114-realtek-driver-startup.md).
It rejects a different PCI function, I/O decoding, enabled bus mastering,
unsupported BAR type/extent and a short mapping before device writes.
`CAP_INFO` hides the physical MMIO base, so matching the capability to BAR2 is
part of the kernel grant contract, not independently verified by this service.
Digital cut must be 1. Only WLMAC power state 0 is accepted before writes and
after shutdown; active or transitional states 1–3 are rejected. The
service does not assume that firmware left in an unknown power-save state can
be reset by the initial cold-start sequence.

The power and indirect-read sequence is adapted under BSD-3-Clause from the
same immutable Linux source above:
[`rtw8852b.c`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b.c)
(`rtw8852b_pwr_on_func`, `rtw8852b_pwr_off_func`),
[`mac.c`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/mac.c)
(XTAL SI commands, power state, coexistence scoreboard),
[`mac.h`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/mac.h),
[`efuse.c`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/efuse.c)
(DDV/DAV reads and logical decoding), and
[`rtw8852b_common.h`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b_common.h)
(the packed board map). No firmware payload is included.

The service holds the baseband in reset, uses the first power-on path before
EFUSE validity/power-trim information is known, reads physical DDV 1216 bytes,
DAV 96 bytes and PHY capability bytes `0x580..0x600`, and decodes each logical
map against its own size (2048/16 bytes). DDV never sets OTP program bits.
Every DDV attempt restores the OTP supply state with a fresh budget, including
failures during enable or read. After any power-on attempt, successful or
failed, the outer cycle attempts the official power-off sequence with another
fresh budget. Errors preserve both the original fault and cleanup failures;
a failed shutdown cannot be reported as successful board access.

The software bounds are: power-on/off 1 second each, each XTAL command 50 ms,
each DDV read 1 second within a 4-second map budget, DAV 3 seconds total with
a nested 10 ms ready/read budget per byte, and DDV restoration 100 ms.
Polling also has a finite iteration limit if the clock stops. Native short waits use calibrated monotonic nanoseconds with a finite
65,536-read guard, rather than the scheduler's 10 ms sleep tick. A tick-only
clock is rejected before device writes; backward/stalled clocks fail explicitly. These are software
polling bounds, not a guarantee that faulty hardware MMIO or a scheduler
cannot stall. Cleanup is attempted; hardware refusal remains an explicit
failure, while the kernel keeps bus mastering disabled.

Board identity requires a unicast nonzero MAC, observed RFE 1 and a present
XTAL trim byte. Country/channel plan and complete raw calibration maps are
retained; they do not authorize transmission, and unknown calibration is not
replaced with invented defaults. No DMA capability, IRQ, `platform::resume`,
firmware download, WLAN service endpoint or radio registration is used. The
binary logs the outcome and sleeps rather than causing an init restart loop.

Validation on `codex/213-DRV-0032-rtl8852be`, commit `a6e69a07`,
2026-10-15: the x86-64 native release build passed; after publication to
`origin/fast-test` merge `0b5bd1bf`, all 16 tests in
`tests/rtw89_power_host.rs` passed (0.05 seconds). They cover grant/clock/cut
refusal, every power-on write failure, independent cleanup budgets, frozen
and late clocks, DDV/DAV limits, malformed logical maps, board identity and
failed/transitional shutdown. No suite was run on `fast-test`.
The actual Ryzen MAC power/EFUSE/shutdown cycle is untested. A QEMU boot with
no RTL8852BE cannot establish it. The earlier Linux identity observation and
parser tests are not evidence for this new device sequence.

## Bounded PCIe command queue (213-DRV-0032)

`fwcmd.rs` implements the channel-12 transport separately from the native
board service. It is adapted under the same BSD option from immutable
[`pci.c`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/pci.c)
(`rtw89_pci_fwcmd_submit`, `rtw89_pci_release_fwcmd`, TXBD initialization),
[`pci.h`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/pci.h)
(eight-byte TXBD, channel registers and `RTW89_PCI_MULTITAG`),
[`txrx.h`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/txrx.h)
and `core.c` (24-byte TXWD), and
[`rtw8852be.c`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852be.c)
(original AX register layout and single-band BDRAM table).

The queue uses 32 descriptors with one position reserved and 40 separate
2 KiB payload slots, retaining the last eight completed commands even after
their descriptor positions are reusable. This matches upstream's distinction
between hardware-index completion and safe payload release. A single,
exclusive 86,016-byte DMA region, aligned to 4 KiB and entirely below 4 GiB,
holds the descriptor page and payloads. The choice of 32 descriptors is a
bounded MIND policy: it is a multiple of upstream's required alignment of 16
and below its configured 256, but it has not been validated on hardware.

Construction rejects unsupported architectures, active PCI mastering,
invalid physical/virtual bounds and alignment before clearing the region.
Installation requires MEM-only PCI Command, both HCI engines off and channel
12 stopped. It bounds the idle/index-reset polls to 100 microseconds each,
programs CH12 count/BDRAM/base and checks readback. It does not enable PCI
mastering or HCI, unstop the channel, initialize other rings or change RF.
The unsafe `start` boundary requires the caller to complete the supported
MAC/DLE/HFC/PCIe setup and enable DMA only after valid rings exist. It verifies
Command (including rejection of inaccessible `0xffff`), HCI, channel, indices
and retained register configuration before accepting packets.

Each submission copies a complete H2C packet or firmware part of at most 2020
bytes behind the TXWD, then publishes the TXBD and 16-bit doorbell. The current
ordering contract is x86-64 coherent WB DMA memory with strongly ordered UC
MMIO and no WC aliases. Release/Acquire fences constrain compiler movement;
x86 WB/UC ordering supplies device-visible ordering. Other architectures are
rejected; neither generic Rust atomics nor host fixtures establish portable
DMA ordering. The caller must retain exclusive ownership of the mapped region
through failures, driver exit and the eight-command prefetch tail until the
kernel/device has independently established quiescence.

Index disagreement, unavailable MMIO, impossible completion or a failed
doorbell poisons the queue. A doorbell failure still retains the possibly
published payload. Drain uses a caller budget of at most one second and a
finite poll count even if the clock stops. Successful drain retains the last
eight payloads; no error, destructor or drain releases DMA memory or claims
that hardware stopped. There is no timeout recovery or restart contract yet.

The focused x86-64 fixture `tests/rtw89_fwcmd_host.rs` models DMA reads at the
doorbell, wrap/full/retention and register/fault bounds. On own-branch commit
`f6caafca`, the x86-64 native release build passed (0.34 seconds, no warnings).
After `fast-test` publication `dd735d0a`, all 14 focused tests passed on
2026-10-15 at 21:01 UTC (0.03 seconds, no ignored tests). No test ran on
`fast-test`. The earlier frozen `a99cc44a` full gate excludes this later slice.
The actual RTL8852BE has not used this queue.
At that library commit the native board service had no DMA capability and
did not call it; the later diagnostic integration is described below.
Firmware authentication/address policy, H2C
encoding, WCPU sequencing and RX/C2H completion are not supplied by this queue.

## MAC download-memory preparation (213-DRV-0032)

`mac::prepare_download` implements `rtw89_mac_dmac_pre_init`'s RTL8852B
memory/flow-control portion from the pinned `mac.c`, `mac.h`, `reg.h` and
`rtw8852b.c`. The entry requires the verified RTL8852BE MEM-only BAR grant,
digital cut 1, active MAC state 1, stopped WCPU, reset baseband, disabled
TX/RX HCI and no reported PCI DMA activity. A failed guard performs no write.
The native board service does not call this new preparation function.

The DLFW profile uses 1024 unlinked 64-byte WDE pages and 64 linked plus
960 unlinked 128-byte PLE pages, accounting for the chip's 196608-byte FIFO.
PLE starts at the 64 KiB boundary. WDE quotas are zero except the WCPU
minimum **48** inherited from the future SCC profile, while its DLFW maximum
remains **0**, as in upstream's explicit override. PLE reserves 16 pages for
C2H and 48 for H2C; other DLFW quotas are zero. Configuration fields preserve
unrelated bits; programmed sizes/quotas get readback before readiness polling.
Each WDE/PLE ready poll has a 2 ms deadline and a finite iteration bound.
Only then is HFC CH12 enabled with pre-cost 40 and full-condition 0;
ordinary data-channel flow control remains disabled. The whole stage has a
10 ms software budget.

Every failure after the first mutation independently attempts HFC disable
and DLE disable, with a fresh 1 ms budget for each and readback. Primary and
both cleanup errors are retained. The caller still owes the full MAC
power-off cycle; successful local cleanup is not a device DMA-quiescence
claim. This function never writes PCI Command, HCI DMA controls, WCPU, BB/RF
or CMAC controls and supplies no firmware, radio or host DMA startup.

Own-branch commit `371d41db` built for `x86_64-unknown-none` (0.31 seconds,
no warnings). After `fast-test` publication `60a100be`, all 11 focused tests
passed on 2026-10-15 UTC (reported 0.00 seconds, none ignored). A prior build
invocation from the repository root used the Linux default target, missing
the service's nested Cargo configuration, and failed in `libmind`; the native
build was rerun from `rtw89_wifi/` without changing code for that error.
The eleven focused fixtures in `tests/rtw89_mac_host.rs` cover the exact download quotas and WCPU override,
pre-write guards, both readiness states, every forward-write fault, failed
readback, cleanup failures and frozen/late/absolute deadlines. Actual Ryzen
MAC/DLE/HFC operation remains untested. PCIe MDIO/DBI configuration, modes,
BDRAM reset, ring installation and external DMA ownership must be integrated
before the command transport can run; this preparation alone is insufficient.

## PCIe preparation with host DMA disabled (213-DRV-0032)

`pcie.rs` ports the RTL8852B-specific path from the pinned `pci.c`, `pci.h`,
`reg.h` and `rtw8852be.c`. `prepare` verifies the previous DLFW MAC profile,
idle indirect interfaces, reset BB/stopped WCPU and PCI MEM-only/HCI-off state
before any write. RTL8852A/C-only workarounds and unrelated PHY generations
are not applied. The fixed observed function has no subsystem BER quirk in
the pinned `rtw8852be.c` entry.

The path applies PCIe power/LDO controls, GEN1 MDIO DPHY delay (register
`0x1b`, 25 microseconds) and AUTOK N (`0x1d`, 4), then disables refclock
calibration in MDIO register `0x30` for the observed link speed GEN1 or GEN2.
Its DBI fallback accesses only PCI vendor/link byte `0x82` and L1 byte `0x719`
through BAR2 `0x1090/0x1094/0x1098`; no generic configuration-write ABI is
introduced. If L1 bit 3 was set, its original byte is saved before the first
attempted change and restored with readback. Failure during that interval
gets a fresh 1 ms restoration budget, distinct from the original error and
the DMA-stop cleanup. A pending DBI operation must complete before its write
data is replaced, including during cleanup. Already-disabled calibration
and L1 are not rewritten.

After the pinned wake/watchdog/debug settings, the implementation stops all
supported TX/RX channels, PCIe I/O and WPDMA with both HCI engines off. Unlike
the complete upstream driver, it has no ordinary RX/TX rings and therefore
leaves these channels stopped throughout this slice. It clears the supported
indices and applies packet RXBD mode, 2048-byte TX/128-byte RX bursts,
multi-tag count 8, 256 ns descriptor intervals and truncated address-info
format. Each index/idle wait is bounded by 100 microseconds; MDIO transactions
by 2 ms and DBI transactions by 200 microseconds, each with finite polls even
if the clock stops. `prepare` has a 50 ms total budget, including its final
readback. There is no PCI mastering, HCI, WCPU or RF enabling operation.

The returned `Prepared` value is consumed by `finish` after the same function's
`Queue` reaches `Installed`. It rechecks the profile and command-ring
base/count/zero indices before writing the exact single-band BDRAM table
(starts 0/5/10/15/20/24/28, with the source's per-channel limits), then polls
BDRAM reset for at most 100 microseconds within a 5 ms stage budget. A guard
failure performs no write. A modified failure independently attempts to
retain channel stops/HCI-off and verifies idle under a fresh 1 ms budget.
Primary, L1-restoration and stop errors are separate; no queue or DMA memory
is freed. The caller must still power the MAC off on failure and retain any
DMA allocation until external quiescence. Success leaves all host DMA off.

Own-branch `4feee434` native x86-64 release build passed (0.64 seconds, no
warnings). After `fast-test` publication `1dee0b3f`, all 16 focused fixtures
in `tests/rtw89_pcie_host.rs` passed on 2026-10-15 UTC (reported 0.00 seconds,
no ignored tests). They model both PHY pages, DBI byte lanes, pending command
completion, every preparation/BDRAM write fault, deadline/restore failures,
ring guards and BDRAM-reset ordering. Independent source review found no
remaining blocking discrepancy. These are not actual PCIe hardware
observations. The frozen `a99cc44a` gate does not contain this slice. The native board service does not call this library stage.
Actual MAC/PCIe preparation, DMA enable/quiescence and firmware/WCPU download
remain unvalidated and must be integrated before a hardware-network claim.

## Hash-pinned WCPU download (213-DRV-0032)

`download.rs` implements a single bounded attempt from the same immutable
BSD-licensed `fw.c`, `fw.h` and `mac.c`: `rtw89_mac_disable_cpu_ax`,
`rtw89_mac_enable_cpu_ax`, `rtw89_h2c_pkt_set_hdr_fwdl`,
`rtw89_fw_download_hdr`, `__rtw89_fw_download_main` and
`__rtw89_fw_download`. It reuses the project's `mind::sha256` implementation;
no new cryptographic dependency or vendor-signature verification is added.

Before any device write, `Verified::verify` checks the complete private file's
1,245,944-byte size and pinned SHA-256, measured cut 1/RFE 1 and the explicitly
nonsecure physical OTP pair `ff/ff` at `0x5ec`/`0x5ed`. Those bytes are already
within `Maps::phycap` (`0x580..0x600`); they must come from the actual board
read, not defaults. Any other security value is unsupported and rejected.
The selected CE image must have offset 96, length 326,616, version
0.29.29.15, commit field `6fb3ec41`, 144-byte header including 64-byte dynamic
extension and exactly these sections, without checksum/redownload flags:

| Type | Decoded destination | Bytes |
|---|---|---|
| 2 | `0x18970000` | 310,992 |
| 1 | `0x18e12400` | 13,432 |
| 9, zero MSS count | `0x18e11928` | 2,048 |

This policy establishes equality to the locally approved pinned file, not a
manufacturer signature or secure-boot support. The opaque `Verified` value
keeps the file immutably borrowed until sending finishes. Host state-machine
fixtures construct synthetic metadata only inside the private test module;
they cannot enter through the production hash policy. The optional private
fixture checks the actual full digest and rejects a one-byte mutation.

The caller must complete MAC/PCIe preparation and command-ring installation
before kernel-enabled bus mastering, leave ordinary RX/TX/WPDMA stopped and
RXHCI off, enable only PCIe I/O/CH12 with TXHCI, then start the queue. `run`
rechecks MEM+MASTER (PCI Command I/O decode may remain enabled), the actual
cut, active MAC, reset BB/WCPU and the DLFW memory
profile, command-only DMA state, exact ring registers and never-used queue.
It resets WCPU/APB/platform, clears communication registers, enables the CPU
clock, sets IDMEM size 2 and boot reason 0, then starts WCPU in download mode.
The header is 8 bytes of H2C plus 80 static header bytes; the dynamic extension
is not sent. Sequence/ACK fields stay zero and part size is 2020. Each section
is sent in at most 2020-byte raw parts through the actual `Queue`; backpressure
reclaims ring positions without reusing the eight retained payloads.

The active PCI Command accepts low bits 6 or 7, matching pinned
`rtw89_pci_claim_device` (`pci_enable_device` then `pci_set_master`) and the
kernel's BAR-derived resume: the observed function has an I/O BAR0 as well
as MMIO BAR2. Every driver access still uses the owned UC BAR2; this stage
performs no PIO or PCI Command writes. The earlier idle/preparation stages
continue to require exact MEM-only low bits 2.

The attempt has a two-second total budget; H2C/FWDL readiness, backpressure
and final drain/status each have 400 ms bounds and finite loop limits even
with a frozen clock. After drain, five 1 ms calibrated delays preserve the
source's 5 ms wait, followed by an actual status-7 check. Status 2/3/4 is
reported separately as firmware checksum/security/cut failure. The final
read must finish within its deadline. There are no blind retries.

Any modified failure poisons the queue and independently attempts all-channel
stop/HCI disable plus bounded idle, then WCPU reset under a fresh budget;
primary and both cleanup failures are retained. Public `stop` gives the same
bounded cleanup after a successful diagnostic. Even successful stop leaves
PCI mastering unchanged and retains all DMA memory, including the final
eight command buffers. Only separately verified kernel quiescence permits
region release. `Ready` means status 7 was observed, not full MAC, calibrated
RF, WLAN service registration or usable networking.

Build and focused host evidence for this library slice are recorded by its
owner. Actual Ryzen security bytes and DMA/WCPU firmware operation remain
unvalidated; native diagnostic integration is described next. The frozen
`a99cc44a` gate does not contain this later library slice.

## Native firmware diagnostic integration (213-KRN-0114)

The native entry reads `data/firmware/rtw89/rtw8852b_fw-1.bin` through its
read-only VFS client into private process pages. Missing, wrong-sized or
wrong-hash files stop before board MMIO access; no firmware bytes enter the
image. VFS open/close calls have two-second IPC timeouts; chunked reads share
a five-second deadline. The exact hash check is repeated with the actual
board's cut/RFE/OTP/image policy after the board-read power-off cycle.

`init` retains a 128 KiB DMA region and passes it in `SLOT_MEM`, along with
the existing decimal PCI device index. The adapter verifies its DMA kind,
4 KiB virtual/physical alignment and full bounds below 4 GiB before queue
construction. The x86 kernel maps DMA as WB and BAR MMIO as UC. The native
adapter uses `ManuallyDrop` for the DMA mapping and never frees or remaps it
after a queue exists, including on setup failure or failed cleanup.

`startup.rs` enforces verification before queue initialization, then MAC
power-on, download-memory preparation, PCIe preparation, ring installation,
BDRAM finish, kernel mastering, command-only activation and one download.
Ordinary packet DMA/WPDMA and RXHCI remain stopped; only channel 12 and PCIe
I/O are unstopped with TXHCI. A successful firmware status 7 is logged with
its version, then independently bounded DMA/WCPU stop always follows.

Kernel `registers_only` disables bus mastering while preserving MEM decode;
an explicit PCI Command read must confirm low bits 2. If either check fails,
no more MMIO is performed. Otherwise MAC power-off gets a fresh one-second
budget, followed by a final kernel quiesce and Command readback. Primary,
DMA/WCPU stop, kernel transition and power-off errors remain separate.
Even complete cleanup does not release the retained DMA mapping or claim
IOMMU containment, drained PCIe transactions or safe reuse. There is no retry,
radio endpoint, packet traffic or native hardware evidence yet.

Own-branch `5f1d940d` builds both the native driver and init. After raw
`fast-test` publication `649fea37`, all ten production startup/activation
host fixtures passed on the own branch on 2026-10-16 (0.00 s, zero
failed/ignored). They cover orchestration and register models, not the actual
PCIe fabric or RTL8852BE power/firmware path. A single cumulative MMIO model
across all library stages remains a separate integration check.

The cumulative `tests/rtw89_pipeline_host.rs` fixture carries one MMIO model
from cold board access through power/MAC/PCIe and command activation, without
reseeding guard registers between stages. Its explicit local companion adds
production pinned-file verification, full transfer/status and stop/power-off,
including missing-H2C-readiness cleanup. Both are compiled but not yet run.
The model synthesizes readiness and consumes doorbells synchronously; it
does not model PCIe timing, ordering faults, real OTP or firmware execution.
There is no new ignored/conditional pass in the host gate. The private check
requires a separately fetched file and writes no payload to a report:

```sh
rustc --edition=2021 tests/rtw89_pipeline_private.rs -o /tmp/rtw89-pipeline-private
/tmp/rtw89-pipeline-private "$PWD/proprietary/firmware/rtw89/rtw8852b_fw-1.bin"
```

## What blocks radio operation

`rtw8852b_chip_info` requires physical EFUSE 1216 bytes, logical EFUSE 2048 bytes (limit 1280), DAV physical/logical regions 96/16 bytes, and PHY capability data at `0x580`, size 128. The resulting board state supplies MAC address, RFE type, country, XTAL trim, TSSI, gain offset, thermal/power/PA calibration. RFE 1 is now observed through Linux; the remaining calibration values have not been collected on the actual Ryzen under MIND. The native board-access service now implements the reads, but its physical operation is unvalidated.

After the board/firmware diagnostic passes its hardware check, validate RFE-specific BB/gain/RF-path/NCTL tables and calibration. The command queue is wired only into the bounded diagnostic above. Full firmware H2C/C2H transport, secure-boot handling, packet rings/interrupts, ownership recovery, channel control, RX/TX and WLAN service integration remain absent. Existing WPA2 logic can be reused only after the radio provides the necessary frame interface. No DHCP, ping or Effector claim follows from these host tests.

## Reproducible host evidence

```bash
rustc --edition=2021 --test tests/rtw89_firmware_host.rs -o /tmp/rtw89_firmware_host
/tmp/rtw89_firmware_host
python3 tests/rtl8852be_fetch_test.py
python3 tests/rtl8852be_probe_test.py
(cd rtw89_wifi && cargo build --release --locked)
python3 scripts/firmware/rtl8852be.py check
RTW89_FIRMWARE="$PWD/proprietary/firmware/rtw89/rtw8852b_fw-1.bin" /tmp/rtw89_firmware_host --ignored
```

On 2026-10-15, 12 synthetic Rust tests, five offline fetcher tests, the `no_std` build and the separate two-cut proprietary-file validation passed. The normal suite intentionally does not require or embed the proprietary file. These checks do not exercise the Ryzen hardware or establish radio functionality.
