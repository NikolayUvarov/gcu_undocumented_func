# RTL8852BE initial channel calibration coordinator

This private continuation joins the actual initial 2 GHz/20 MHz channel
calibrations on one PHY attempt and one firmware/command/RX session. It has no
native caller. The three consumed `CalibrationTransmit` authorities still have
no production constructor: firmware startup, a selected channel, a stopped
scheduler and Linux's country observation do not establish calibration power
or platform SAR/EIRP permission.

The pinned source dispatch is
[`rtw8852b.c:643–660`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b.c#L643).
For a first channel only, MCC's software slot is zero. The coordinator requires
all three engine authorities to match the actual attempt, command session,
channel and policy version before issuing a connection notification. It then
runs:

1. Actual owned connection RFK begin.
2. Production channel RXDCK and IQK through the existing IQK wrapper.
3. The BTC owner's 30 ms preservation window when its actual accepted BTINFO
   snapshot reports A2DP; other profiles do not acquire an invented delay.
4. Production TSSI using the actual successful IQK result.
5. Another source BTC preservation window.
6. Production DPK using the actual successful TSSI result.
7. Matching connection RFK end, then normal RF-notify-MCC H2C through the same
   command sequence and owned RX/ACK engine.
8. A final live PHY and owned-session check before returning the opaque result.

The source preservation helper is
[`coex.c:11887`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/coex.c#L11887).
The 8852B does not select `RFK_NTFY_MCC_V0`: the source feature table enables
that feature for 8852C. Its first-slot notification therefore contains five
little-endian words `[channel, channel, 0, 0, channel]`, with category 2,
class 10, function 2, explicit ACK flags false and the normal periodic receipt
policy. See
[`fw.c:6173`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/fw.c#L6173).
DMA consumption and required receipt are not firmware-applied RF evidence.

A 15 s whole-work limit covers the continuation. Before a child engine begins,
the coordinator reserves its existing maximum work budget: 6 s for the IQK
wrapper including channel RXDCK, 4.5 s for TSSI, and 3 s for DPK. Owner callbacks
receive at most 100 ms, with absolute checks between stages. The existing
finite register-poll bounds remain unchanged. Independent error cleanup is
outside that work budget and retains each child's primary error and shutdown
failures. No bound promises progress under indefinite preemption.

A failed stage cannot run the following calibration or notify MCC. Independent
radio shutdown precedes the sole matching connection-end cleanup. An uncertain
END or MCC send is never retried. Backing DMA remains owned, ordinary DMA TX
and the packet scheduler remain stopped, and the result is not association or
packet-transmit authorization. A subsequent consumer must revalidate the live
attempt/session and current policy; identity equality alone is not readiness.

The host continuation fixture uses the existing cold PCIe/OTP/firmware/MAC/RX
model and extracted initial-PHY RF/LTE/DACK device. It adds write-triggered PLL,
NCTL and PMAC/CW responses, without reseeding production guard registers between
stages. Calibration policy is test-only. The existing five initial-PHY cases
and their exact H2C trace remain registered; the new coordinator harness is
registered separately. Native build passed in 1.45 s without warnings and the
seven new cases compile. Execution follows raw publication on the owner branch;
these device reactions are models, not hardware calibration evidence.

## First published execution and trigger correction

After raw publication of `9ad494eb` in `c38df1dd`, all seven new cases failed
in the model before calibration: the baseband table writes `0x9c0` with
`0x980` TX grant disabled, but the model treated that initialization pulse as
PMAC transmission. A captured backtrace locates the failure in `Phy::baseband`,
not in the coordinator. The preserved five initial-PHY cases passed, as did
two cold PCIe cases and 186 MAC/RX/session cases (one existing private-firmware
case ignored). Logs remain under `/tmp/mind-channel-*-results-9ad494eb.log`.

The model now requires the source PMAC TX grant and packet-enable bit before
a trigger creates a transmission reply; raw path, power, count and period
assertions remain intact. NCTL command replies likewise require the start
bit, excluding the source table's `0x8000=8` initialization writes. No
production code or successful-calibration assertion changed. Corrected
execution remains pending its own raw publication.

## Corrected execution

After raw publication of `fbe42d48` in `5cbf1825`, the saved corrected host
binary passed all seven new coordinator cases in 0.08 s, with no ignored
cases. The exact source build passed in 0.01 s. The successful GEN1/GEN2 model
retains one downloaded firmware, one queue/RX session and one PHY attempt
through all three engines; its accepted A2DP event supplies both 30 ms windows.
IQK uncertainty, missing TSSI report, bad DPK quality and unconsumed MCC DMA
prevent readiness and later-stage work. The original seven failures and
source-derived trigger correction above remain part of the evidence.

The earlier same-change affected checks passed: five preserved initial-PHY
cases (0.14 s), two cold PCIe cases (0.01 s), and 186 MAC/RX/session cases
(1.05 s), with one existing private-firmware case ignored. These are host
models, not physical RF or WLAN traffic evidence.
