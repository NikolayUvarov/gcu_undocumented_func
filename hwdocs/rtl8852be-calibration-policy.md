# RTL8852BE initial source calibration policy

The private provider in `rtw89_wifi/src/calibration_policy.rs`, exposed inside
`phy::channel`, resolves the supported source calibration profile and grants
one initial IQK/TSSI/DPK permission set. This continues
[213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md). The native scoped-input
consumer remains dependent on the [KRN platform-policy request](../issues/requests-KRN.md#device-bound-platform-power-policy-for-the-ryzen-wlan-driver).
No radio endpoint, packet scheduler or native calibration caller is enabled
by this slice. Actual radiated power and WLAN operation are not measured here.

## Inputs and permission

`Resolved::resolve` is a crate-private input boundary for the future trusted,
device-scoped consumer. Its `Rule` must come from the actual location's channel
policy and include the conducted/EIRP relation after antenna/system gain;
these values are not inferred from an observed Linux country or a file name.
`PlatformInput` distinguishes unknown, invalid, explicit limits and checked
absence. Checked absence means no *additional SAR* restriction, using the
source maximum before country/default caps; it does not mean unlimited RF
power, zero physical antenna gain or successful collection after an error.
The provider does not evaluate AML, authenticate arbitrary configuration or
create an MMIO capability. The kernel-owned input contract remains separate.

The only compiled source profile is cut 1/RFE 1/internal-PA RTL8852B, initial
2 GHz/20 MHz. Actual Board identity must agree with the OTP maps; both PHY-cap
front-end bytes 0x5ec/0x5ed must match the supported erased/internal-PA profile.
The existing source calibration decoder handles the real maps, including its
specified erased-field defaults. The advisory `power_k_valid` flag is not
turned into a new certification requirement. Country conflicts, disabled,
passive, unsupported channels, invalid power values and unknown inputs refuse
admission. The actual TSSI HT20 MCS7/1TX/non-BF source table and both path/1TX
platform ceilings must permit its maximum requested setting, 12 dBm.

`channel::run` retains the exact applied Policy, including raw and selected
country, board channel-plan byte, effective caps, regulatory conducted/EIRP
caps, power group, channel, access and version, plus a hash of actual Board
identity and all three OTP maps. A version number alone cannot substitute
another policy with different values. This fixed-size hash is identity binding,
not independent evidence that a caller acquired the Board from hardware.
The owned board-read/native pipeline supplies that provenance.

`Resolved::grant` consumes the resolved input and requires the actual `Tuned`,
PHY attempt, unpoisoned Session, same-session firmware `DefaultTables`, and
actual BTC `Initialized`. The source profile uses the already hash-pinned
0.29.29.15 firmware without appended override elements. A final live retained
RX/command validation has a 10 ms budget. After it begins, the Tuned object
records the attempted admission even if validation fails; resolving the same
inputs again cannot mint another permission set or retry uncertain admission.

The three resulting `CalibrationTransmit` values are neither Clone nor Copy
and are specific to IQK, TSSI and DPK. They retain the exact policy and board
binding, and existing engines consume them. The ordinary coordinator still
checks its actual prerequisites, BTC arbitration, finite RFK sequence and
cleanup. These permissions authorize only the supported source routine; they
do not authorize arbitrary RF register writes or ordinary packet transmission.

## Direct source evidence for TSSI units and rate

The production register sequences remain derived from the BSD option of Linux
[`05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`](https://github.com/torvalds/linux/tree/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89).
`rtw8852b_rfk.c:3410-3433,3485-3565` sends raw powers 48/20 through the same
manual PMAC setter. `rtw8852b_common.c:1435` selects the HT20 MCS7 PLCP table;
`1523-1530` writes the signed nine-bit BB field 0x4594[30:22]. This therefore
uses the **MCS** power-limit section, not OFDM, even though both are non-CCK.
The pinned FCC channel-9 entries differ: OFDM raw 78, MCS raw 76.

The hardware unit is independently explicit in original Realtek 52B source,
pinned to vendor tree `52f1fa3aa5171301c8b009756a1cf7a53a9d91d0`:

- [halrf_tssi_8852b.c:1796-1816](https://github.com/lwfinger/rtw8852be/blob/52f1fa3aa5171301c8b009756a1cf7a53a9d91d0/phl/hal_g6/phy/rf/halrf_8852b/halrf_tssi_8852b.c#L1796)
  computes the centi-dBm representation as `power_tmp * 100 / 4` and forwards
  that same raw value to the 52B helper. Alignment at lines 1434-1435/1519
  uses the same helper with 48/20.
- The helper at lines 89-110 forwards unchanged through
  `halrf_pmac.c:126-143`, `halrf_interface.h:56`, `hal_api_bb.c:677-686` and
  `halbb_pmac_setting.c:133-155` to the actual 8852B setter.
- [halbb_8852b_api.c:1233-1256](https://github.com/lwfinger/rtw8852be/blob/52f1fa3aa5171301c8b009756a1cf7a53a9d91d0/phl/hal_g6/phy/bb/halbb_8852b/halbb_8852b_api.c#L1233)
  writes the low signed nine bits unchanged to that same field and its getter
  sign-extends nine bits. Thus 48/20 mean **12/5 dBm requested power**.

These vendor GPL-2.0 files were read only for hardware-interface facts; no
vendor implementation or tables were copied. Source hashes are
`26cc50db77c3d161ea60ea4fa6640e6a3672190794c49a0f7d060b8923e832de`
for `halrf_tssi_8852b.c`,
`6b7fadd6512edb410d006f79fb49daba79414ccb679ccfb06663ca5c4d562b12`
for `halrf_interface.h`, and
`a82cd788e03d7deecee8a3401599db0363b852e938be9b73f83609204812f691`
for `halbb_8852b_api.c`. The arithmetic and pointer-free admission provider
are original MIND code. Existing Linux-derived attribution remains in
[THIRD_PARTY.md](../THIRD_PARTY.md).

## Permission is distinct from a measured emissions guarantee

IQK/DPK use source-specific RF modes and gain codes, not this PMAC quarter-dBm
field. The normal reference code 63/+24 dBm relationship does not establish
an IQK/DPK antenna-output bound. No such number is invented or required here.
Linux `core.c:490-518` programs channel/power before RFK, and
`rtw8852b.c:645-662` executes the supported IQK/TSSI/DPK sequence without an
independent numerical IQK/DPK certificate. Trust in this exact chip, board
calibration, firmware and source sequence is explicit in this first profile.

MC-2.3/3.3 require explicit contracts and authority distribution, MC-6.1 defines
failure outcomes, and MC-12.1/12.3 require honest evidence boundaries. Neither
those clauses nor DRV-0032 adds independent RF certification before executing
the supported driver sequence. A future measured-output guarantee would need
its own evidence. The present permission does not claim that guarantee.

## Validation and remaining work

Fourteen focused cases passed on source `202bba961c512f6b16cf501c0158bf813189a402`
after native release build (3.54 s) and raw publication
`882bd73575306964bb53e552cb2d073ae8f64ab3` on 2026-10-16 (0.03 s, no
failures or ignored cases): checked-absent/unknown/invalid inputs,
board/PA/OTP consistency, country/passive/channel refusal, quarter- and
half-dBm boundaries on both paths/1TX, actual MCS versus OFDM limits, advisory
calibration data, exact policy replacement, changed OTP/session, repeated
admission, foreign firmware provenance, failed PHY/live RX and a cumulative
cold device through actual provider, channel RFK and MCC. Existing literal
Tuned fixtures use a test-only constructor; they cannot mint production
permission. The same own-branch run also passed the nine affected complete
harnesses: PHY 283, IQK 189, channel RFK 210, post-MAC 178, BTC init 178,
DPK 203, pipeline MAC 188, BTC Session 178 and initial PHY 183 (4.31–4.55 s
each). These totals contain shared models and are not a count of distinct
tests. Each retains the existing separately fetched private-firmware case
as ignored; no new test was skipped. This change does not alter that file
or its downloader/hash policy.

The native KRN scoped input, actual selected country/platform policy and
physical Ryzen calibration/traffic evidence remain open. The pure provider
is not evidence that those inputs have already been acquired.
