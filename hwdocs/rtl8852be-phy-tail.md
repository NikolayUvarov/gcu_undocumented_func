# RTL8852BE initial PHY DM tail

Part of [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md). This is a
crate-private register engine, not an integrated WLAN service, regulatory
decision or permission to transmit. The actual BTC initialization and RFK
interleaves remain mandatory before native integration.

## Source and order

The BSD-3-Clause alternative of Linux commit
`05f7e89ab9731565d8a62e3b5d1ec206485eeb0b` supplies the chip-dispatched
sequence in [rtw8852b_common.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b_common.c)
and register masks in [reg.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/reg.h).
Their SHA-256 hashes and licence notice are recorded in the
[PHY source ledger](rtl8852be-phy.md#exact-source-and-table-selection) and
[THIRD_PARTY](../THIRD_PARTY.md). No firmware payload is copied.

After the completed initial RXDCK, `phy::tail::run` performs:

1. `__rtw8852bx_set_txpwr_ctrl` / `set_txpwr_ref`: initial reference zero,
   equal path offset, MAC `0xd200[27:10]`, then OFDM A/B and CCK A/B at BB
   `0x5804/0x7804/0x5808/0x7808`. The packed source value is `0x02b27000`.
2. `thermal_trim`, then `pa_bias_trim`: decoded global programmed flags,
   RF43 thermal field and RF60 2G/5G bias fields, A then B. An individually
   erased byte retains the source-defined value when its peer is programmed;
   both erased bytes skip that trim operation.
3. `__rtw8852bx_bb_cfg_txrx_path`: RX path selection and initial HT/VHT/HE
   limits; per-subband gain compensation and signed gain offsets; the exact
   2G/B-or-AB BTG branch; selected TSSI power reset; both RF mode fields;
   final firmware NSS limits; MAC mode zero.

The 8852BT external-loss branch does not apply to 8852B. The unused 5 GHz
gain-compensation subband is never selected. This stage does not configure a
channel, fill regulatory/SAR/TPE power-limit tables, associate, or release any
ordinary TX DMA or scheduler queue. RF mode and calibration writes follow the
source initial PHY sequence while packet TX remains stopped.

## Inputs and failure handling

The entry requires `RxdckDone`, the original Board identity, a DM prefix from
the same nonzero PHY attempt and band, and opaque `phycap::Capabilities` from
an actual successful firmware query. `PhyIo::command_session()` defaults to
zero; an adapter must provide the actual owned Queue session, equal to the
capability token. A missing response or a token from another queue cannot
select guessed NSS. The source's successful-zero-field fallback stays explicit
in `Capabilities`. RX antenna zero means unspecified; the effective source
path becomes AB, while a one-antenna response selects B and one stream.

The live post-DACK guard is still required, including the real owned RX permit
if RX is active. Ordinary packet TX, WPDMA and scheduler queues remain stopped.
The 100 ms absolute deadline includes the live guard, register access and all
SWSI polls; final completion is checked against the original clock. Masked
writes and RF trims are read back. Invalid inputs refuse before MMIO writes.
Every possibly applied failure poisons PHY and independently attempts the
existing 400 ms chip-disable sequence, retaining the original fault and any
cleanup fault. Command/RX DMA memory remains owned by the caller; this engine
never frees or revokes it. Success yields `PathsConfigured`, which describes
only this stage and cannot authorize channel or packet transmission.

## Evidence

Ten new host cases compile in the mandatory PHY harness. They use the real
firmware-query parser and private capability constructor with a modeled mailbox,
then exercise the production tail. Cases cover source order, erased/mixed
trims, signed gain arithmetic, all supported 5 GHz subbands, actual reply NSS
resolution, foreign session/board/prefix refusal, live TX/phase refusal, every
applied write failure, ignored-write readback and independent cleanup, and a
late final readback. The existing PHY, BTC-init and post-MAC harnesses compile
with the merged report/capability modules; no new skip is introduced.

Native build passed in 0.88 seconds. Execution awaits raw publication. These
are host model checks, not evidence of analog calibration quality or physical
Ryzen association, ping or Effector connectivity.

The next [channel/RFK dependency plan](rtl8852be-channel-plan.md) records the
missing actual board/regulatory inputs and exact channel/power/calibration order.

## Executed host evidence

After raw publication `539b588e`, the exact saved own-branch `5477b455`
binaries passed all ten new tail fixtures on 2026-10-16 MSK. The full
PHY harness passed 156 cases (0.96 s), BTC-init passed 85 (0.90 s), and
post-MAC passed 76 (0.87 s), with no failure. Each harness retains one
pre-existing ignored test requiring the external proprietary firmware file.
The harnesses share cases; these totals are not distinct additional tests.
Independent source review found no blocker. This evidence is synthetic and
does not establish country, calibrated power or radio operation on Ryzen.
