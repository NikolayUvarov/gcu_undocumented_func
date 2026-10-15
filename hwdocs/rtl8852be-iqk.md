# Initial RTL8852B host IQK

Issue: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md).
The private library implements initial non-DBCC, PHY0, 2 GHz/20 MHz,
wideband IQK for the already selected RFE1/cut1 board. It has no native caller
and does not establish calibrated radio quality, association or packet traffic.

## Source and licence

The selected BSD-3-Clause option is recorded in THIRD_PARTY.md and
LICENSES/BSD-3-Clause-rtw89.txt. The source is Linux v6.19 commit
`05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`,
`drivers/net/wireless/realtek/rtw89/rtw8852b_rfk.c` (SHA-256
`c4613e3d2755f14649154a1c8393bf72ae215e8b683219a0763a033b224c715f`)
and the same commit's `reg.h`. Relevant functions are `rtw8852b_iqk`,
`_doiqk`, `_iqk_by_path`, `_iqk_lok`, `_txk_group_sel`, `_rxk_group_sel`,
`_iqk_check_cal`, `_iqk_restore` and their clock/backup helpers.
`scripts/hardware/rtw89_iqk_tables.py` checks the complete source hash before
extracting exactly 32 setup and 18 restore entries. It never accesses a
network or device; `--check` reproduces the checked-in tables.

## Admission and operations

The private wrapper checks ChannelTuned state, the PHY attempt, actual command
session, the completed BTC owner, channel and policy identities. It consumes
an engine-specific opaque CalibrationTransmit authority. No production code
can currently construct that authority: ordinary channel power limits and
zero packet scheduler do not establish that LOK/TXK calibration emissions
meet platform/EIRP/SAR constraints. Tests use an explicitly modeled policy
admission, never a native bypass. Each authority admits at most one attempt.

Production channel RXDCK runs first on the same bus. IQK then preserves the
source outer IQK START/STOP, zero scheduler pause/restore and RF_AB one-shot
START/STOP around each path. Real BTC ownership is required by the production
callback; host callback fixtures model it separately. Both paths use initial
MCC slot zero. The later connection-RFK/MCC coordinator, TSSI, DPK and channel
firmware notification remain separate prerequisites.

The register engine preserves three BB and eleven per-path RF backups, the
non-DBCC setup table, preset and clocks, up to three LOK bias attempts,
four wideband TX and four RX groups, source diagnostic/result fields,
restore command, restore table and saved registers. BB offsets add 0x10000;
RF accesses use existing bounded SWSI/direct helpers. Completed results retain
actual LOK coordinates, all command quality flags and result snapshots, bound
to the attempt/session/channel/policy. Unlike upstream void callers, nonzero
quality flags, including vbuffer and restore flags, cannot issue Completed.
This stronger admission is explicit, not a claim of measured RF quality.

## Bounded failure behavior

Each NCTL command has the source 8,200 microsecond poll followed by 200
microseconds; LOK retries and group loops are finite. IQK has a 3.3 second
aggregate ceiling, including two one-second mailbox ceilings and six
100 ms BTC ceilings. The preceding channel RXDCK has its own 2.5 second
ceiling; the wrapper checks a total six-second success deadline.

A trigger is considered pending before its possibly applied write. Until
completion is observed, no subsequent calibration/restore trigger is issued.
Unknown completion skips register restoration and records `nctl_uncertain`;
independent chip radio shutdown is attempted immediately, before potentially
waiting on scheduler restoration. The caller must destroy the
failed attempt and perform full device shutdown; there is no claimed NCTL
abort or safe continuation. Known-idle failures attempt table/register
restoration within a fresh 20 ms budget. A failed scheduler exchange is never
retried; a known completed pause can be restored once to the same zero state.
Scheduler status, chip shutdown and matching nested/outer notifications keep
separate cleanup errors. RF cleanup precedes releasing coexistence ownership.
No DMA allocation or PCI mastering state is freed or changed by this module.
Post-RXDCK or final validation failure also attempts independent radio shutdown.

## Evidence and remaining work

Eleven IQK fixtures and two offline extraction/shape fixtures are registered
in the host gate. They cover exact command/group paths and backup restoration,
LOK bounded retry/coordinates, every command's quality flag, uncertain
completion, frozen time, BTC denial/late response, uncertain scheduler exchange,
every applied register-write failure, final-read deadline/cleanup errors,
single-attempt STOP and identity/admission boundaries. They are compiled,
not executed, pending own-branch commit/build and raw publication. The device
model completes register commands ideally and does not simulate RF physics.
Full gate, complete native calibration policy, calibration chain integration
and physical RTL8852BE observations remain open.
