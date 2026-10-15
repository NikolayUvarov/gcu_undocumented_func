# RTL8852BE native initial PHY diagnostic

Issue [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md); roadmap H / 213,
D / 550; Constitution MC-6.1, MC-12.1, MC-12.3. This is initial hardware setup,
not a completed WLAN service or evidence of physical-device operation.

## Actual composition

`initial_phy::diagnose` requires the board read from this owned function,
capabilities belonging to its successful firmware startup, the installed
command/RX DMA objects and the live Receiving token. It consumes Ready once
and creates one Session. Its normal command sequence and ACK correlation are
shared by RF mirror commands and all subsequent BTC commands.

The fixed order is BB and RF tables; actual BTC normal/no-link initialization;
validated DM prefix; NCTL; RCK; actual BTC-arbitrated DACK; actual scheduler
mailbox and BTC-arbitrated RXDCK; board-specific DM tail and receive paths.
BTC completion is revalidated at the stage boundaries. Every stage retains
its existing finite budget. A failure stops progression and preserves both
its own primary/cleanup fields and the owner's last RFK cleanup record.

The Session and Phy then end. Only the existing report-register tail follows;
the consumed Ready is not returned and cannot recreate sequence zero. The
result contains observations, not a token for channel tuning or transmission.
Native startup always performs independent RX/radio/command stop, kernel
mastering disable/readback, bounded power-off and quiesce afterward. The full
8 MiB allocation stays retained even when cleanup fails.

This order composes the pinned-source adaptations already attributed in
[the PHY notes](rtl8852be-phy.md) and [BTC notes](rtl8852be.md). It does not
introduce copied firmware or a second command/mailbox implementation.
The public library API requires the same exclusive device ownership and
mandatory shutdown as its native caller; it is not a reusable live service.

## Deliberate limits

The native entry still rejects unsupported board identity and already-active
MAC state. Initial DACK/RXDCK are distinct from channel IQK/TSSI/DPK, actual
country/channel/SAR/antenna-power admission, scan, association and data TX.
Those later stages are not called here. No radio endpoint is registered.
Passing this diagnostic on a real Ryzen would establish only the logged
initialization stages; it would not establish a working network connection.

## Validation boundary

`tests/rtw89_initial_phy_host.rs` extends the existing cold-device model with
write-triggered SWSI, LTE and calibration responses. Firmware download, MAC,
RX, H2C ACKs, BTC policy and scheduler exchanges use the production paths on
one device and the actual programmed descriptors. The wrapper does not seed
successful firmware/MAC guards or supply a fake calibration grant. Synthetic
result registers do not establish analog calibration quality or timing.

Five focused fixtures compile: GEN1/GEN2 success followed by shutdown,
missing RF command ACK, busy BT admission, NCTL/RCK/DACK failure cutoffs and
final report-write cleanup. The existing twelve startup fixtures cover the
new fifteenth stage and independent cleanup errors. Execution follows the
own-branch build and raw publication; results will be recorded here.

The first execution at source `1a99bece`, after publication `180f9559`, passed
four cumulative cases and all twelve startup cases. The success fixture failed
its guessed `submitted > 10` assertion after completing the actual pipeline.
For the modeled BT-disabled state, source initialization sends exactly two RF
mirror commands and eight BTC commands; RFK keeps the same no-link policy and
does not send redundant policy commands. The fixture now requires the exact
ten category/class/function headers and sequential sequence bytes instead of
that guessed lower bound. Production behavior is unchanged. Corrected-case
execution is pending its own raw publication.


Corrected source `c1582e24`, built in consolidated ASTRA `48036ca6` and
published in `fast-test` `b169a275`, passed all five focused cumulative cases
(0.08 s, zero failures/ignores, 167 imported cases filtered out). Together with
the separately executed twelve startup cases, this covers the stated model
composition and cleanup. Two independent source reviews found no additional
composition defect. The full x86 image build passed in 1 s; no physical-device
initialization, calibration quality, channel service or network traffic was
observed, and the overall gate remains open.
