# RTL8852BE initial channel register stage

This continues [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md).
The code is a private, bounded initial 2.4 GHz/20 MHz managed-channel engine.
It is not called by the native service and does not authorize packet or
calibration transmission. Physical channel tuning, power accuracy, RFK and
WLAN connectivity have not been validated.

## Inputs and boundaries

`phy::channel::run` requires the completed initial DM/RXDCK/path stage, the
same PHY attempt and command session, and the opaque `DefaultTables` token
minted after successful download of the exact pinned firmware. The token
establishes that this file has no appended override elements; it does not
replace live MMIO/RX ownership validation or platform power constraints.

Private `Policy::resolve` accepts an explicit versioned rule for one channel
and resolved platform caps. It rejects disabled frequencies, channel 14,
other bands/bandwidths, an unknown platform limit, invalid signed units and
a country conflicting with recognized board OTP. Unknown/worldwide OTP
requires an externally selected real country rule; no country is invented.
The rule's conducted-power cap must already include the platform's
EIRP/antenna/system-gain relationship. It may not exceed the EIRP cap in this
initial scope. Platform caps include actual SAR effects for 1TX, 2TX and
individual paths; missing SAR is never interpreted as unlimited power.
Per-path SAR values determine the source power-reference difference.

There is **no native policy provider** yet. This internal constructor is not
an attestation that its caller has obtained real platform/regulatory evidence.
The recorded Linux DE snapshot is not automatically consumed. Passive-only
permission remains explicit in the result and cannot authorize TSSI-generated
transmissions or later association. A future public entry must bind an actual
policy authority and completed BTC/channel-RFK state, not expose these
private fields or accept a boolean readiness claim.

## Source sequence and bounded errors

The engine retains the original zero scheduler mask, through the same
exclusive firmware-mailbox callback for pause and restore. It performs PPDU
disable, both TSSI holds, ADC disable, 40 microseconds and BB reset; MAC channel
fields; source CCK/SCO, gain/RPL, bandwidth, receive masks and reset; both
analog and direct RF channel/bandwidth paths; RXBB LUT; all power registers;
then source PPDU/ADC/TSSI/BB restoration and zero scheduler restore.

The power image contains 58 writes: 11 packed by-rate words, five packed rate
offsets in one word, eight CCK DFIR words, triangular shape, 20 ordinary limit
words, 12 RU limit words and five path-reference writes. Unused 20 MHz page
slots remain source zero. The actual 1TX/2TX caps constrain applicable signed
half-dBm limits. Legitimate all-ones power words are read back as data rather
than treated as a disconnected device. Excessive path differences are
rejected before the source unsigned TSSI reference can wrap.

One 2.2-second absolute deadline covers both mailbox phases, live guards and
all register work. The RF LPF wait retains its source 1 ms ceiling and finite
iteration count. Source LCK recovery is finite; persistent unlocked state is
an explicit failure, rather than a log followed by success. Temporary LDO,
MMD, trigger and LUT gates are tracked before writes and restored under a
separate 2 ms cleanup budget, followed by independently bounded 400 ms chip
BB/RF disable. A completed pause may be restored to zero once under a
separate 1-second mailbox budget; an uncertain pause/restore is never retried.
The original failure and cleanup errors are retained separately. DMA memory
ownership and device quiescence remain with the caller.

The successful state is `ChannelTuned`, not calibrated or TX-ready. The
source same-band initial scope avoids pretending that a missing band-change
BTC/TSSI transition has occurred. Channel RXDCK/IQK/TSSI/DPK and MCC/BTC
notifications still have to run in their actual source order.

## Primary source and offload review

All adapted register sequences use the BSD-3-Clause option of Linux commit
[`05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`](https://github.com/torvalds/linux/tree/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89).
Source hashes are retained in the [PHY](rtl8852be-phy.md),
[DM tail](rtl8852be-phy-tail.md) and [power-policy](rtl8852be-channel-plan.md)
records; attribution is in [THIRD_PARTY.md](../THIRD_PARTY.md).
The controlling anchors are `rtw8852b.c::rtw8852b_set_channel_help`,
`rtw8852b_common.c::__rtw8852bx_set_channel_{mac,bb}`, the RFK file's
`_ctrl_ch/_ctrl_bw/_rxbb_bw`, and `phy.c` power-page serialization.

The exact RTL8852B RFK dispatch has no reviewed firmware-offload alternative
for IQK/TSSI/DPK: `rtw8852b.c:645` calls the host implementations at
`rtw8852b_rfk.c:3757–3850`. `fw.c:825–885` assigns no RFK-offload feature to
RTL8852B 0.29.29.15; its RFK IQK/RXDCK/PRE_NOTIFY version selections concern
RTL8922A. Generic `H2C_FUNC_RFK_*` opcodes do not establish support on this
chip. The engine does not substitute such commands.

## ACPI evidence collection

Pinned `acpi.h:136–139` names `WRDS` (static SAR), `RWRD` (dynamic tables),
`RWSI` (active selector) and `RWGS` (geographic adjustment). `acpi.c:80`
resolves them relative to the PCI WLAN device's ACPI node, then evaluates
them without arguments. Static DSDT/SSDT inspection must first identify that
device path; a matching string elsewhere is insufficient. Evaluation itself
is not part of this read-only evidence collection.

The driver prefers a recognized WRDS, otherwise RWRD, then applies RWGS and
the dynamic selector when available. HP CID 0x5048 adds a 3 dB 2TX reduction;
RT CID 0x5452 uses the source per-path tables. `RTAG` is the separate antenna
gain method. TAS has `_DSM` UUID `D2A8C3E8-4B69-4F00-82BD-FE8607803AA7`,
revision 0/function 5, but actual RTL8852B dispatch has `support_tas=false`.
Missing kernel debug messages or absent string-search output do not establish
SAR NONE. Reading debugfs `txpwr_table` calls `rtw89_leave_ps_mode` and is not
used under the no-NIC-state-change diagnostic restriction.

The read-only Ubuntu operation `20261015230428.464521382_000170` observed
PCI `03:00.0=10ec:b852`, no `firmware_node/path` entry, permission denied for
DSDT/SSDT under the ordinary user, and no matching SAR/ACPI lines in the
existing kernel journal. It evaluated no AML and read no `txpwr_table`.
These are collection results, not a resolved platform SAR policy. The source
`acpi.c` SHA-256 is
`75567534eb96a822de667b136b0248770c250bc769138c44d6afdbbda2c8343d`;
`acpi.h` is
`90a87fc398bbd90b043c6c06779cae4e746bfde203717b81b7f8a967926354a9`.

## Validation status

The native library/service builds without warnings in 1.05 seconds. All six
affected host harnesses compile. The eight power-image and 12 channel-stage
fixtures are compiled but not yet executed; execution waits for publication
of this source commit to raw `fast-test`, then occurs on the owner branch.
The fixtures cover signed power packing and bounds, source ordering,
identity/policy refusal, finite synth/LPF failure, original LDO restoration,
uncertain mailbox failures, readback errors and absolute deadlines.

After raw publication `71b3a770`, the exact saved `5bb78d89` binaries ran on
the owner branch: PHY 170 passed, BTC init 87 passed, Session 91 passed,
post-MAC 78 passed and cumulative MAC 84 passed. Each inherited harness had
one ignored private-firmware case; none failed. The power-image harness had
seven passes and one failure, preserved in
`/tmp/mind-channel-channel_power-results.log`. The failed expectation wrongly
treated CCK20/CCK40 as adjacent non-BF values: `phy.h:477` actually interleaves
non-BF/BF, and the pinned CCK and OFDM BF entries are zero. Only 2TX MCS has
the nonzero BF entry used here. Production packing matches `phy.c:2488`; the
fixture now independently checks `[cap,0,cap,0]` and the distinct 1TX/2TX MCS
BF values. Its corrected execution remains pending publication.

RFK can transmit while the ordinary scheduler remains stopped. The private
opaque `CalibrationTransmit` binds the PHY attempt, firmware session, channel,
policy version and named IQK/TSSI/DPK engine, and refuses passive channels.
It has no production constructor: actual calibration gain/PMAC level units,
regulatory and platform SAR/EIRP admission are still unresolved. A test-only
model constructor exercises identity refusal without granting native rights.

After raw publication `8ebd2da1`, the corrected power fixture passed all eight
cases (0.00 s), and the saved updated PHY/admission binary passed 171 cases
(0.89 s), with one inherited private-firmware ignore and no failures. The
original 7/8 failure remains recorded above. These are register-model and
packing results, not physical power measurements or calibration authorization.
