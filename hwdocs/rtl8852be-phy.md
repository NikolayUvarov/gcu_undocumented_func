# RTL8852BE RFE-1/cut-1 PHY table stages

This is an executable library slice of [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md), not a complete radio startup. The native board-read service does not call it. It has no WLAN endpoint and cannot establish association, IP traffic or Effector connectivity.

## Exact source and table selection

All adaptations select the BSD-3-Clause option from Linux commit `05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`, directory `drivers/net/wireless/realtek/rtw89`, with Realtek's notice in [LICENSES](../LICENSES/BSD-3-Clause-rtw89.txt). No firmware binary is included.

| Source | Purpose | SHA-256 |
|---|---|---|
| [rtw8852b_table.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b_table.c) | Selected BB, gain, radio A/B and NCTL tables | `8b9b5fc32628037d8dec86cff976913164723d87abe38fd09d229c57b509b42f` |
| [phy.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/phy.c) | Conditional selection, gain decoding, SWSI/direct RF, firmware mirror, NCTL preamble | `ddb89306110634a6cf0c06fb6a38fbfc01bd46fe81bd2dd3875a923a4674734a` |
| [reg.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/reg.h) | MAC/BB/RF guard and access definitions | `a302f390b451ce1dcdf8dd1d5149910a71cb8621b309b274eb5b1ce060335333` |
| [mac.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/mac.c) | Full DMAC/CMAC definitions and single-band TX-power register mapping | `ce5c09696ba6dcdc2003ce28803052fd3238c9da55188046e3e6e1ed82658055` |
| [fw.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/fw.c) | RF H2C layout | `64c86436e8e0f7f9ecd4364d2d2c48be4549a0448b3ce738eee7e8c25e5875ce` |
| [rtw8852b_common.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b_common.c) | Actual RTL8852B BB/RF dispatch, TX-power unit and common BB reset | `34dcc96967f408ba2b7589af26f49e8ba26c0a822934811e599a2974b86a3afb` |
| [rtw8852b.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b.c) | Chip BB-reset wrapper and ops dispatch | `644f8f7f3900181fb1bf0acbd17adeacafd5ba77a63ed9dd83c751a0c705179a` |

The offline [extractor](../scripts/hardware/rtw89_phy_tables.py) checks the source hash before parsing. It accepts only the exact RFE-1/cut-1 headline `0x00010001`, not the upstream fallback to a different cut/RFE. It rejects unsupported flow control, unknown gain fields, delays/bypasses requiring new review and unexpected table sizes. The generated file preserves every selected write, including duplicates and their order:

- BB: 1013 writes; gain: 66 software records, never MMIO writes.
- RF A: 947 writes; RF B: 932 writes. Each has 294 digital records mirrored to firmware in one 1176-byte page.
- NCTL: 1320 writes, preceded by its clock/reset setup and bounded ready check.

To reproduce from a separately downloaded local source:

```sh
python3 scripts/hardware/rtw89_phy_tables.py /path/to/pinned/rtw8852b_table.c --check
```

This tool never downloads or changes a network device. PHY offsets add the AX MCU base `0x10000`; RF direct windows are BAR2+`0x1e000`/`0x1f000`, and SWSI is BAR2+`0x10370`/`0x1174c`. The table loader requires a mapped BAR of at least `0x20000` bytes. The original Ryzen report observed a 1 MiB BAR2, so its physical extent is sufficient; [KRN-0114](../issues/213-KRN-0114-realtek-driver-startup.md) must retain the complete same-function grant and confirm its runtime size before future activation. This library requests no ABI change.

## Integration contract

`PhyIo` is an exclusively owned same-function adapter with a bounded command-ring transport. Every stage rechecks PCI identity/Command, cut 1, board RFE 1, MAC power active, WCPU enabled with actual firmware status 7, full DMAC/CMAC and clocks, BB/RF access and single-band mode. RTL8852B chip dispatch requires SPS ZCDC_H=1, WLRF AFC_AFEDIG bit17 and PHYREG_XYN cycle0xe; it does not use the generic MAC WLRF0x8282 sequence. Successful chip enable must also have set XTAL80/81 to0xc7 before these read-only guards. The DLFW-only MAC state is rejected. Active PCI Command must have MEM and MASTER set; the kernel may additionally decode BAR0 I/O, which this MMIO-only library never accesses. Only the command channel may run: ordinary RX/TX/WP channels and all MAC scheduler queues must be stopped, and ordinary DMA/protocol TX must be idle. The caller must also exclude concurrent firmware calibration, scan, autonomous transmission and register writers.

Source order in `core.c::rtw89_core_start` and `phy.c` requires these separate stages and interleaves:

1. Complete verified firmware download/full MAC initialization, BTC power-on and chip BB/RF enable before `baseband()`.
2. `baseband()` now performs the BB writes, six single-band CMAC0 TX-power-unit writes, software gain decoding and 15 RTL8852B BB-reset writes in source order under one deadline. It returns `Gain` only after the reset completes; the caller retains those software defaults for later channel work. The CMAC0 power addresses do not add the PHY MCU offset. Reset preserves unrelated fields and includes the required 1 µs delay. These defaults are not regulatory power limits or permission to transmit.
3. `radio()` applies path A, mirrors its digital words, then applies and mirrors B. `send_rf_page` must use H2C type 0, category 2, class 8/9, function page 0, with explicit receive/done ACK flags false and the normal advancing H2C sequence/periodic receive-ACK policy (`fw.c::rtw89_h2c_pkt_set_hdr` requests receive ACK every fourth command). It must copy the payload and drain hardware command consumption/PCIe TX busy within the supplied remaining budget. Hardware consumption is not firmware acknowledgement.
4. Complete BTC initialization and PHY dynamic-management initialization before `nctl()`.
5. `TablesLoaded` still requires RFK initialization (DPK state, RCK, DACK, RXDCK), board trims, channel-dependent IQK/TSSI/DPK, coexistence coordination, channel selection and regulatory/SAR/TPE limits. It does not authorize transmission. Missing calibration or policy cannot be replaced by these generic gain defaults.

The private state enforces BB→radio→NCTL and blocks reuse after failure. It does not verify the external chip interleaves; the caller remains responsible for them. `bus_between_stages()` exists for these required steps, not for concurrent data traffic. The library never enables PCI bus mastering, HCI engines, data queues or MAC scheduler transmission.

Each stage has a 100 ms total software budget, including both RF paths and their H2C calls; SWSI polls have a 30 µs sublimit and finite iteration guard. NCTL readiness has a 1 ms sublimit. A failed modified stage attempts the chip-dispatched BB/RF disable sequence (clear AFC bit17, assert BB reset, read/clear XTAL80/81 low3 while preserving other bits) with an independent 400 ms total budget (two XTAL paths, each with four finite 50 ms idle/read/write/readback ceilings), preserving the primary and cleanup errors. It waits for an existing XTAL command to finish before issuing another, so a failed path cannot overwrite a pending command. This does not undo table writes, stop/reclaim command DMA, power down the MAC or prove RF silence. The caller must retain all DMA memory, stop the transport and perform verified outer shutdown; a fresh object cannot substitute for that shutdown. Guard refusal performs no writes. These limits rely on bounded register/transport methods and a calibrated monotonic clock; they do not bound physical MMIO faults or arbitrary scheduler delay.

## Validation scope

After publication of `f4766604`, all 20 production-code host fixtures and four extractor fixtures passed, as did pinned-source regeneration. This corrected the earlier test expectation that incorrectly placed RF register 0 before the source table's register 5. The added TX-power/reset slice builds natively and all 24 fixtures compile; execution of the revised fixtures awaits publication to `fast-test` and runs only on this branch. Four new fixtures cover the exact 21-write interleave, masked-field preservation, an applied failure at every added write, the final-write deadline and reset-delay overrun. The fixtures cover exact data fingerprints and write order, guards, signed gain decoding, indirect/direct RF windows, H2C error propagation, shared/late deadlines, frozen polling, failed cleanup and NCTL readiness. Four offline extractor fixtures cover exact selection and rejection. No native integration, actual Ryzen register sequence or radio behavior is established by this slice.

Independent review found and corrected an initial generic-MAC dispatch mistake before native integration or test execution: this slice now follows `rtw8852b.c::rtw8852b_chip_ops` into `rtw8852b_common.c` for the actual chip BB/RF state and cleanup.
