# RTL8852BE RFE-1/cut-1 PHY and initial RFK stages

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
| [rtw8852b_common.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b_common.h) | Packed logical board calibration layout | `f58fead1913d8bf4c9453e47bbe12241f96bb68109d1806eccf9b11c8ce54666` |
| [rtw8852b_common.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b_common.c) | Actual RTL8852B BB/RF dispatch, TX-power unit and common BB reset | `34dcc96967f408ba2b7589af26f49e8ba26c0a822934811e599a2974b86a3afb` |
| [rtw8852b.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b.c) | Chip BB-reset wrapper and ops dispatch | `644f8f7f3900181fb1bf0acbd17adeacafd5ba77a63ed9dd83c751a0c705179a` |
| [rtw8852b_rfk.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b_rfk.c) | DPK backoff, RCK, DACK and initial RXDCK sequences | `c4613e3d2755f14649154a1c8393bf72ae215e8b683219a0763a033b224c715f` |
| [rtw8852b_rfk_table.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b_rfk_table.c) | DACK/AFE/diagnostic register tables | `ebc6840d2e15c7a9f12fb7d7121ca19bb8acff93524d3902a7ce892e0714fc21` |
| [coex.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/coex.c) | Actual BTC RFK arbitration/notification prerequisites | `c38d8a42a57300c0c376217d3472e2a3176760346ab44b8c21cca6b98c63a11c` |
| [coex.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/coex.h) | DACK type, START/STOP and PHY/band map | `164a6975c1fcf24eb71bf015cc9998404e0a627c19197ce33b3e8e6daadafd3e` |
| [phy.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/phy.h) | DM bins, page numbers, DIG and CFO constants | `9ee6c68363c286a842b50237eb322094a6c6051d0e4e8c242b4cd7fe249ff321` |
| [core.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/core.c) | Startup order and RTL8852B cut-specific DIG support | `41ea6a22a4ff78035364def00c70edb49d821f2d4cb12869f1a0a87e6ee54886` |
| [core.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/core.h) | Initial PHY/DM software settings and thermal scale | `48ebe2404ae8f57ff3260687580d6f867d3b2725bdf93db01ceb9ab3a8c9f138` |
| [mac.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/mac.h) | RTL8852B indirect XO/XI addresses and masks | `911b0258c04474c9acc6505c253ac63ff82483f7e24e645095bee9e59b3ea4d6` |

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

`PhyIo` is an exclusively owned same-function adapter with a bounded command-ring transport. Every stage rechecks PCI identity/Command, cut 1, board RFE 1, MAC power active, WCPU enabled with actual firmware status 7, full DMAC/CMAC and clocks, BB/RF access and single-band mode. RTL8852B chip dispatch requires SPS ZCDC_H=1, WLRF AFC_AFEDIG bit17 and PHYREG_XYN cycle 0xe before DACK and 0xf after it; it does not use the generic MAC WLRF0x8282 sequence. Successful chip enable must also have set XTAL80/81 to0xc7 before these read-only guards. The DLFW-only MAC state is rejected. Active PCI Command must have MEM and MASTER set; the kernel may additionally decode BAR0 I/O, which this MMIO-only library never accesses. The command-only path retains its stopped-RX contract. Active RXQ/RPQ additionally requires the actual owned receive-session permit described below; ordinary TX/WP channels and all MAC scheduler queues remain stopped, and ordinary DMA/protocol TX remains idle. The caller must also exclude concurrent firmware calibration, scan, autonomous transmission and register writers.

Source order in `core.c::rtw89_core_start` and `phy.c` requires these separate stages and interleaves:

1. Complete verified firmware download/full MAC initialization, BTC power-on and chip BB/RF enable before `baseband()`.
2. `baseband()` now performs the BB writes, six single-band CMAC0 TX-power-unit writes, software gain decoding and 15 RTL8852B BB-reset writes in source order under one deadline. It returns `Gain` only after the reset completes; the caller retains those software defaults for later channel work. The CMAC0 power addresses do not add the PHY MCU offset. Reset preserves unrelated fields and includes the required 1 µs delay. These defaults are not regulatory power limits or permission to transmit.
3. `radio()` applies path A, mirrors its digital words, then applies and mirrors B. `send_rf_page` must use H2C type 0, category 2, class 8/9, function page 0, with explicit receive/done ACK flags false and the normal advancing H2C sequence/periodic receive-ACK policy (`fw.c::rtw89_h2c_pkt_set_hdr` requests receive ACK every fourth command). It must copy the payload and drain hardware command consumption/PCIe TX busy within the supplied remaining budget. Hardware consumption is not firmware acknowledgement.
4. Complete BTC initialization and the pre-NCTL prefix of `phy.c::rtw89_phy_dm_init` before `nctl()`: chip BB setup, monitoring, DIG/CFO/wrapper/EDCCA, antenna/RFE and other preceding steps. NCTL and RFK are themselves inside that upstream function; completing all DM before NCTL would be circular. This caller-owned prefix remains unimplemented in this library.
5. `rck()` requires `TablesLoaded`, rechecks hardware guards, initializes single-band DPK backoff and runs RCK on paths A then B. The caller retains fresh TSSI/MCC software state as required by `rtw8852b.c::rtw8852b_rfk_init`. The returned RCK values and DPK software gain do not imply full calibration. RCK itself has no BTC START/STOP wrapper in the pinned source; DACK and RXDCK do, and remain separate mandatory stages.
6. `dack(band)` requires `RckDone` and a mandatory `DackIo::btc_rfk` implementation. It runs the source AFE, DRCK, ADDCK A/B, backup/reload, DACK A/B and diagnostic sampling sequence, returning results only after RF restoration and BTC STOP.
7. `rxdck(band)` requires `DackDone`, the post-DACK register phase and mandatory BTC/scheduler callbacks. It completes only the initial single-band sequence with both source TSSI-mode flags still false.
8. `RxdckDone` still requires board trims, channel-dependent IQK/TSSI/DPK, coexistence coordination, channel selection and regulatory/SAR/TPE limits. It does not authorize transmission. Missing calibration or policy cannot be replaced by these generic gain defaults.

The private state enforces BB→radio→NCTL→RCK→DACK→RXDCK and blocks reuse after failure. It does not verify the external chip interleaves; the caller remains responsible for them. `bus_between_stages()` exists for these required steps, not for concurrent data traffic. The library never enables PCI bus mastering, HCI engines, data queues or MAC scheduler transmission.

The BB, radio, NCTL and RCK stages each have a 100 ms total software budget, including both RF paths and their H2C calls; SWSI polls have a 30 µs sublimit and finite iteration guard. NCTL readiness has a 1 ms sublimit. Analog RF reads preserve the source 2 µs trigger delay and finite 30 µs idle/completion sublimits; masked writes preserve other RF bits through the SWSI hardware mask or direct-window RMW. Each RCK completion poll has a 30 µs total sublimit, and both paths/DPK setup share one 100 ms stage deadline. A failed RCK path first attempts RF5 restoration/readback under an independent 200 µs ceiling; a busy SWSI command cannot be replaced. Primary, RF5-restore and chip-disable errors remain distinct. Unlike the upstream logging-only RCK timeout path, this implementation aborts, skips later paths and never marks RCK complete after timeout. A failed modified stage attempts the chip-dispatched BB/RF disable sequence (clear AFC bit17, assert BB reset, read/clear XTAL80/81 low3 while preserving other bits) with an independent 400 ms total budget (two XTAL paths, each with four finite 50 ms idle/read/write/readback ceilings), preserving the primary and cleanup errors. It waits for an existing XTAL command to finish before issuing another, so a failed path cannot overwrite a pending command. This does not undo table writes, stop/reclaim command DMA, power down the MAC or prove RF silence. The caller must retain all DMA memory, stop the transport and perform verified outer shutdown; a fresh object cannot substitute for that shutdown. Guard refusal performs no writes. These limits rely on bounded register/transport methods and a calibrated monotonic clock; they do not bound physical MMIO faults or arbitrary scheduler delay.

## DACK and BTC boundary

`dack.rs` follows `_dac_cal` and its helpers in pinned `rtw8852b_rfk.c`.
The [offline DACK extractor](../scripts/hardware/rtw89_dack_tables.py) hashes
`rtw8852b_rfk_table.c`, selects exactly 11 tables/86 operations and rejects
unknown effects, invalid fields, addresses or unreviewed delays. Run it with
`/path/to/pinned/rtw8852b_rfk_table.c --check` to compare committed output.
Those BSD tables are calibration instructions, not proprietary firmware.

BTC notification is mandatory and typed: calibration DACK=4, START=1/STOP=0,
path map 0, PHY0 map bit 4, and the current 2.4/5 GHz channel-band map in bits
7:6. The caller supplies the actual initialized band, not an invented default.
START success must mean that `_chk_wl_rfk_request`-equivalent arbitration,
WLRFK scoreboard and coexistence actions have allowed the calibration.
An H2C receipt or firmware ACK alone is insufficient. The callback must also
implement STOP/release after a possibly applied START failure. No successful
native callback is provided here; actual BTC and the DM prefix remain blockers.

One 400 ms deadline covers callbacks and both calibration paths. Each callback
receives at most 100 ms and its elapsed time is checked after return. DRCK,
ADDCK and each DACK part retain 10 ms finite polling ceilings. Failures preserve
the primary error, independent BTC STOP error, per-path RF0/RF5 restoration
errors (one 1 ms total restoration ceiling) and BB/RF-disable error (400 ms).
STOP is attempted once, including after partial START, and is not blindly
retried. On failures before STOP, RF restoration and BB/RF disable run while
START arbitration is still held; STOP follows that independently bounded cleanup.
If STOP itself has already failed, cleanup still runs without retrying STOP.
DMA memory must still remain owned until verified outer shutdown.

Success restores the saved RF0 values and sets RF5 bit0 as the source does.
Failure restores saved RF0/RF5 values before disabling BB/RF. AFE writes the
source `PHYREG_SET=0x0f`; later stages must validate that post-DACK state rather
than reusing the pre-DACK `0x0e` guard. The source requires both completion bits
on path A but either bit on path B; the engine preserves this asymmetry and
returns the actual two-bit completion masks. Calibration arrays and five
100-sample signed DC averages are returned only after all stages and STOP
succeed. Diagnostic averages are observations, not additional success limits.
This is not channel calibration, association or permission to transmit.

## Initial RXDCK and scheduler ownership

`rxdck.rs` follows pinned `rtw8852b_rfk.c::_rx_dck`, `_wait_rx_mode` and
`rtw8852b_rx_dck` for the initial `rfk_init` call. Its BTC notices use type 5,
START/STOP and the same initialized PHY/band map. `RxdckIo::scheduler_zero`
must delegate to the shared `runtime_mac::scheduler_zero` mailbox helper;
it cannot create a second writer or bypass the actual firmware exchange.
Both pause and restore send only the already-zero scheduler mask. Initial
source TSSI software modes remain false; the channel-change path with active
TSSI is not implemented here.

One 2.5 s total deadline includes two scheduler exchanges (each at most the
source 1 s ceiling), two BTC callbacks (each at most 100 ms), both paths and
all completion checks. Each RF mode wait has the source 5 ms finite ceiling
and 2 µs poll interval. Each path saves RF5/fine, clears the RF93 calibration
field, triggers RF92 bit0 from 0 to 1, waits 1 ms and restores fine/RF5. Source
success leaves RF0 in RX mode 3. `Completed` reports the sequence/restoration;
the source has no RXDCK-completion-bit poll here, so neither that return value
nor the tests establish measured analog calibration quality.

Any failed attempt remains poisoned. After a successful pause, failures
attempt exactly one zero-only scheduler restore. An error or late result
from either mailbox call forbids another exchange: cleanup instead checks
actual CTN/PTCL status without retrying the uncertain command. Saved RF0,
fine and RF5 restoration share an independent 1 ms budget; scheduler restore,
status (80 ms), BB/RF disable (400 ms) and BTC STOP retain distinct failures.
RF cleanup and chip disable precede releasing BTC arbitration. DMA memory is
retained until verified outer quiescence. The shared crate-private phase guard
and `Phy::mark_failed` also let the BTC-init interleave reuse the same live
checks and poison a modified failure without copying the register contract.

## Typed owned receive-session guard

`PhyIo::receive_permit(timeout_us)` defaults to refusal. An adapter admitting
active RX must return the concrete `post_mac::ReceivePermit` borrowed from
`Receiving::validate`, which rechecks the actual command and receive objects,
non-reused session IDs, live addresses/counts/indices, firmware/MAC state and
masked host interrupts. A bool or raw matching register masks cannot construct
this permit. The adapter must retain the original same-device/exclusive-owner
contract; the permit is an observation, not lasting hardware authorization.

The PHY guard recognizes only the command-only stopped-RX configuration or
command plus owned RXQ/RPQ. The latter callback has a 10 ms ceiling with an
independent elapsed-clock check, after which HCI/STOP, ordinary TX/WPDMA busy
and CTN/PTCL are checked again. No DMA, interrupt or transmit register is
written. Both pre-DACK and post-DACK BB/RF phases retain their exact checks.
An expired or failed callback prevents the PHY stage from starting. The native
adapter must delegate to the same sole ring owners; this change supplies no
native activation or BTC initialization.

Six integration fixtures use the real queue/ring/session implementation and
post-MAC activation model, not a constructed-success permit. They check live
RX/command traffic, default refusal, corrupted live rings/indices, ordinary
TX/WPDMA/phase refusal, late/backwards clocks and post-permit STOP changes.
After publication `08a4c784`, the PHY harness passed 125 cases, post-MAC 57
and BTC-init 66 on the own branch `2a41a980` (1.13 seconds each). All six
new cases pass in every harness; shared module cases are repeated, not unique
additional evidence. Each harness retains one previously ignored private
firmware-file fixture, requiring separately fetched proprietary bytes. No new
conditional skip is introduced. These are host models, not hardware results.

## PHY attempt identity

Every `Phy::new` allocates a bounded, nonwrapping software session identity from
`io::dma_session`, independently of the bus address. Allocation failure creates
an already-failed PHY before register access. The crate-private `attempt()`
lets the BTC owner bind its prepared configuration to this exact attempt as well
as to the actual firmware/queue/receive sessions; pointer reuse cannot establish
that relationship. This identifier is not hardware readiness or a BTC token.
After publication `1006c136`, both new attempt fixtures passed within the
127-pass PHY harness at `323269dc` (1.00 seconds, zero failed, one existing
private firmware-file fixture ignored). Same-address reconstruction and
allocator exhaustion perform no MMIO. Native build passed in 0.80 seconds.

## Contiguous initial DM prefix and remaining order

The crate-private `phy::dm` engine implements the contiguous register prefix
between actual BTC `ntfy_init(NORMAL)` and NCTL. It requires `RadioLoaded`
and the same live pre-DACK guard. There is no public entry or successful
BTC-ready stand-in: hardware-only `ConfigPrepared` does not establish completed
BTC software/control/policy initialization. Integration must obtain the actual
private-constructor `btc::Initialized` token before exposing this engine.

The single 300 ms deadline covers both RF thermal triggers/200 µs waits,
chip `bb_sethw` (sound bits, all 128 MACID power-limit words and signed gain
baselines), CCX/IFS bins, all 15 AX PHYSTS pages, initial DIG, board XO/XI
and DCFO, EDCCA collision distance and UL-TB bandedge snapshot. XTAL operations
retain their finite 50 ms ceilings and never replace an outstanding command.
Readback mismatch and late final reads fail. Modified failure poisons `Phy`
and independently attempts chip BB/RF disable under 400 ms, retaining DMA.
A successful result contains observations and initial nonzero software settings;
it does not advance the public PHY pipeline or enable periodic tracking.
The integration must create fresh zeroed packet/statistics/tracking counters,
keep both TSSI software modes false and preserve the unassociated state.

The chip dispatch skips NHM (`support_noise=false`), unsupported IGI gain
programming (`support_igi=false`), AX-null BB-wrapper/channel-info hooks,
null RFE-GPIO/RFK-hardware hooks and antenna diversity (two physical RF paths).
CCK packet detection remains required on cut 1. Its initial zero IGI RSSI gives
CCK threshold byte `0x80` for every supported starting bandwidth; no channel
or regulatory choice is fabricated here. The private engine's caller-supplied
band selects only the source unlinked DIG software profile.

The remaining integration order is explicit:

1. Actual BTC initialization and this DM prefix, then NCTL and existing
   RCK/DACK/RXDCK with actual BTC arbitration and shared firmware scheduler.
2. Source `set_txpwr_ctrl`, physical-board thermal/PA trims and `cfg_txrx_path`.
   The board decoder must preserve programmed/unprogrammed sentinel semantics.
   RX path and NSS require the actual firmware PHY-capability register exchange;
   the physical OTP `phycap` byte array is a different input. Chip maximum NSS
   is not a substitute for a firmware response.
3. MAC PPDU/PHY-report/RTS tail and owned RX-channel validation. Active RX
   requires an opaque live session permit, not relaxed raw DMA masks.
4. A validated regulatory channel, actual BB/RF channel setup and the required
   channel RXDCK/IQK/TSSI/DPK plus BTC intervals and rate/power limits, followed
   by MAC roles/CAM/security, data TX rings, authentication and association.

None of these steps is replaced by an ACK-only BTC callback or by these model
fixtures. Actual Ryzen calibration, association, ping and Effector remain open.

## Board calibration decoder

`calibration::decode` interprets the actual `Board` logical map and 128-byte
physical map starting at OTP `0x580`. It supports only the identified cut-1,
RFE-1 device. The logical TSSI blocks start at `0x210` and `0x23a`; thermal
references are `0x2d0/0x2d1`, and signed gain offsets retain the chip band order.
The physical map provides the exact power-calibration marker, descending TSSI
trim bytes, thermal/PA fields and signed gain compensation. It performs no
MMIO and does not change the supplied maps.

Sentinels follow the source globally: all sixteen erased TSSI trim bytes become
zero, but one programmed byte preserves every other `0xff` as signed -1.
Thermal/PA writes are omitted only when both respective bytes are erased;
when one is programmed the other path retains the source's `0xff`-derived
setting. Gain validity is likewise global, with signed -1 preserved for erased
entries and the unused 5 GHz subband index 2 left zero. Unprogrammed logical
TSSI and thermal references are not silently replaced. These data do not
substitute for the firmware PHY-capability reply or authorize transmit power.

Nine pure fixtures cover independent packed offsets, signed fields, global
sentinels, every thermal-byte encoding, unused subband, exact marker and foreign
cut/RFE refusal. After publication `1006c136`, all nine own-branch decoder
fixtures passed (zero failed/ignored, 0.00 seconds) at `34e5f9cc`; native
build passed in 0.80 seconds. The private [DM tail](rtl8852be-phy-tail.md)
consumes these values after RXDCK, with actual firmware capabilities and
DM gain baselines bound to the same PHY attempt.

## Validation scope

Initial `59a77177` execution passed 18/19 Rust fixtures and failed the RF-prefix assertion because it incorrectly placed RF register 0 before the source table's register 5. After publication of `f4766604`, all 20 Rust and four extractor fixtures passed, as did pinned-source regeneration. After publication of `78cf274a`, all 24 Rust fixtures passed, including exact TX-power/reset order, field preservation, every added write failure and deadline failures. After publication of `0d6c66bf`, all 34 RCK/RF/PHY fixtures passed. After publication of `7c6d5ecc` and registration `a8418545`, all 44 Rust and three DACK extractor fixtures passed, as did pinned DACK-table regeneration. Ten added cases cover RF field validation/readback, frozen/pending SWSI, state/guard refusal, DPK threshold branches, both RCK paths, timeout restoration, distinct cleanup errors and late completion. The fixtures cover exact data fingerprints and write order, guards, signed gain decoding, indirect/direct RF windows, H2C error propagation, shared/late deadlines, frozen polling, failed cleanup and NCTL readiness. Four offline extractor fixtures cover exact selection and rejection. No native integration, actual Ryzen register sequence or radio behavior is established by this slice.

Independent review found and corrected an initial generic-MAC dispatch mistake before native integration or test execution: this slice now follows `rtw8852b.c::rtw8852b_chip_ops` into `rtw8852b_common.c` for the actual chip BB/RF state and cleanup.

The ten DACK cases cover BTC ordering/refusal/late completion, captured calibration data, every stage timeout, the source S1 flag asymmetry, every applied register-write failure, independent cleanup errors and the whole-stage deadline. Three extractor regressions cover parsing, unsupported effects and source-hash refusal.

After publication of `abe309ea` in fast-test `b50f52c0`, all 56 Rust fixtures, three DACK extractor fixtures and pinned DACK regeneration passed. Twelve added cases cover exact two-path writes and timing, phase/queue guards, BTC and uncertain scheduler failures, source RF-mode timeout, late completion, every applied write failure, cleanup ordering and distinct cleanup errors. Native activation and actual Ryzen radio validation remain pending.

After publication of `727aab95` in fast-test `badae1bc`, the exact compiled DM binary passed all 68 Rust fixtures (0.11 seconds, none ignored). Its native build passed (0.48 seconds). Twelve new cases cover the full order, all MACID words, PHYSTS compression, CCX units, signed/zero observations, DIG/cap branches, stuck or mismatched XTAL, every applied write error, independent cleanup and late total deadlines.
