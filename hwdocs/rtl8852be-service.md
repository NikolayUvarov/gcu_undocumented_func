# RTL8852BE private wlanhw 1.1 adapter

Issue: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md); MC-2.3,
MC-3.3, MC-6.1, MC-12.1, MC-12.3 and Appendix B.6.

`rtw89_wifi/src/service.rs` adapts an actual, borrowed `DataSession` to the
existing generated [wlanhw 1.1](../idl/wlanhw.wit) protocol. It remains
crate-private and uncalled by the native executable. It cannot construct a
calibration permission, choose a country, resolve missing platform input,
associate with an AP or create a connected firmware role. The native
coordinator and KRN endpoint/configuration grants remain separate prerequisites.

## Request behavior

- `info` reports the actual board MAC, only the already admitted channel,
  20 MHz 2.4 GHz legacy CCK/OFDM capability, and a 2346-byte MPDU limit.
- Setting that same channel succeeds without changing hardware. Every other
  channel, including zero, is invalid. This is not multi-channel scanning.
- `set-filter` uses the actual active CAM command, done ACK, DMA completion,
  software policy and epoch retirement. Successful change also discards frames
  already serialized into the service's previous-epoch queue.
- `send` copies the generated decoder's private request into the real TX owner
  and defers its reply while subsequent IPC and RX work continue. Only the
  checked final firmware count, after RPQ and descriptor consumption agree,
  becomes `sent.attempts`; group addressing retains `acked=false`.
- `receive` uses the real retained RX descriptor and `frames::from_packet` to
  remove the FCS and write the existing eight-byte header. Signal zero means
  unmeasured. Channel and scan flag describe the admitted receive epoch.
- `attach` and `kick` return invalid, as the existing BCM adapter does: this
  slice uses `receive` and implements no shared ring.

Association, WPA2, keys and upper networking remain the station service's work.
The software filter keeps the existing raw-six-byte address semantics. A
selected BSSID does not fabricate a connected-role event.

## Bounded ownership and scheduling

Each `step` receives at most one IPC message with a 2 ms timeout, progresses
one actual RX/TX operation with a 100 ms device budget, and dispatches at most
one request. Both paths run under continuous traffic. A filter command has a
100 ms absolute device deadline. Progress retains the normal command budget
for actual BTC reconciliation; the DataSession intersects it with any pending
TX deadline. A step with a filter request can use both bounded phases, up to
200 ms plus IPC and scheduling before failure cleanup. The source-defined independent failure
cleanup still applies. These budgets do not promise a hard physical scheduling
latency or behavior of an unresponsive device.

There is one outstanding transmit call, four deferred word-only wait calls,
and four serialized frames of at most 2354 bytes. The actual DataSession inbox
remains separately bounded. Waiters reply when frames become available; a
full waiter set or its one-second lease expiry replies zero. No saved reply
is abandoned on an ordinary stop or error: pending send replies failed and
waiters reply zero. A timed-out client may make the reply fail; its local
capability and mapping are still released by the real wire implementation.

The two fixed receive slots, 9 and 10, must be reserved and empty before
construction. A deferred buffer send keeps its received memory capability
and mapping in one slot; the next request uses the other. Ordinary buffer
requests finish before that slot is reused. Constructor checks refuse an
occupied slot. The service endpoint itself must be restricted to the WLAN
station by init; no new badge or authority is invented here. The existing
wire/kernel capability, mapping and saved-reply semantics remain in the TCB.

The generated `wire::Call` owns an attached or malformed buffer capability.
The adapter does not explicitly drop `attach` a second time. Stop and Drop
finish every deferred reply and call the real independent data-mode shutdown.
`stop` returns the borrowed data owner, primary failure and shutdown result;
it never releases DMA. Outer RX/WCPU/power/PCI quiescence remains mandatory.

An unhandled retained C2H/RPQ or other control record is an explicit terminal
error with the record still held by the owner, not a Wi-Fi frame or a silently
discarded event. Known BTC event reconciliation is the separate active-owner
continuation. Its absence is not masked by this adapter.

## Validation boundary

Fifteen new fixtures execute through the cold Board, verified synthetic
firmware, capabilities, real MAC/RX/PHY/calibration, station/filter and
DataSession owners. Only kernel calls and physical hardware responses are
modeled. Tests use the production generated IDL decoder, encoder and wire
resource lifetime implementation, including deferred calls and client buffers.

The cases cover exact info and receive bytes; same-channel refusal behavior;
pending TX versus other requests and alternating receive slots; actual filter
epoch changes; bounded waiters, cancellation and save-reply exhaustion;
exactly-once received-capability release; malformed requests; continuous IPC
and RX progress; missing final TX report; retained unknown control; and Drop.
The dedicated harness is explicitly registered in `scripts/host_tests.sh` with
`--cfg rtw89_service`, so its child cases are not omitted by normal host CI.

Source `ed3808cf37d90b19c5fce5def1a6bdabb34bf390` built natively from the
crate directory in 1.70 s before publication. After raw merge
`d4e55fccc207a33edb502a14d38c7f2b95e50441` was pushed, its saved host harness
passed all 15 new service cases in 0.05 s, then the full affected harness
passed 279 cases in 1.26 s on 2026-10-16. The one inherited private-firmware
case remains explicitly ignored in the normal run; this execution did not
claim that separately fetched payload check. No test was weakened or run on
`fast-test`. The earlier build invocation from the repository root selected
the Linux target and failed on target-specific libmind output; the corrected
native crate-directory build above is the build evidence.

This slice does not establish a native endpoint, RF reception/transmission on Ryzen,
association, DHCP, ping or sustained traffic. The existing hardware acceptance
criteria remain open.

## Runtime coexistence integration regression

The follow-up merges source `f7f808b6` and exercises the actual `Service::step`
through a BT enable transition whose real H2C acknowledgment arrives after
15 ms. The Info reply must succeed, the modeled hardware must have delayed
that actual acknowledgment, and a subsequent real Send must finish with the
checked one-attempt result and no retained capability leak. This checks the
100 ms progress budget at the service boundary rather than merely calling
DataSession directly. Source `40f93daf45ac87d0dcbfd632bc521ee9415f7fa1` built
natively in 1.86 s and its saved host harness compiled before raw publication
`4f58ad98f8fb5e8ade91ecc3c19788afe6a82e75`. After that publication on
2026-10-16, all 16 service cases passed in 0.05 s and the full updated harness
passed 294 cases in 1.29 s, with the same one private-firmware case ignored.
The earlier 15-case evidence above remains unchanged.

## Production coordinator composition

One follow-up cold case calls the production `connection::run`, then creates
`Service` directly in that coordinator's callback. It sends generated-wire
Info, a real active filter/CAM change and deferred TX, verifies the checked
one-attempt reply and exactly-once capability release, and stops the adapter.
The original coordinator fixture additionally checks report-tail registers,
`encoded_rts=0x02ff`, mandatory outer shutdown and retained DMA cleanup. There
is no guard reseeding between those stages. The ordinary four connection
cases stay unchanged; the composed child is included by the explicitly
registered service harness.

Source `9a000fa2da9b624533ebbdf201560d754e053b48` built natively in 3.79 s
and the saved service harness compiled before raw publication
`4a4fa150bc430660efe696d302041a9219c36711`. After that publication on
2026-10-16, the composed case passed in 0.03 s and the full harness passed
299 cases in 1.27 s, with the one inherited private-firmware case ignored.
This is evidence for that own source revision, not the later H2C changes in
the aggregate candidate or native endpoint/radio behavior.

## Receive-only service before calibration (213-KRN-0123's gate, 2026-11-05)

The country reaches the radio through `set-country` on its own endpoint, so the endpoint comes before any calibration. KRN and DRV agreed on two lines:
- `[RTW89] SERVING WLANHW: RECEIVE ONLY UNTIL A COUNTRY IS APPLIED`, the gate for `init`;
- `[RTW89] DATAPATH ADMITTED ON CHANNEL n`, after a country and that channel's calibration. Since 801-DRV-0071 an `unknown` platform word is held back, not enforced (801 S5), and a station that sends before it gives a country gets the world rules (`00`).

`receive_service::serve` is the first. It runs when `init` lends an endpoint in `SLOT_SERVICE`; without one, the driver keeps its bounded listen. It does the listen's setup:
- `listen::open`, then `listen::prepare_receive` on the caller's live PHY: the initial PHY, one passive channel, the report tail and the scan filter.

It then serves `wlanhw` 2.0, receive-only (as of 2026-11-08):

| Request | Answer |
|---|---|
| `info` | `channels` 1–13 before a country, those its limits leave after; `supported` 1–13 |
| `set-channel` | any of those channels, tuned passively (`channel::run` under `Policy::passive`) and answered after the tune; others `invalid` |
| `set-filter` | the station filter applied in software over the scan filter. When it ends a scan (`all-beacons` set, then cleared), the service logs `[RTW89] SCAN: n PASSIVE TUNES, THE LONGEST t US; k FRAMES KEPT, d DROPPED` |
| `receive`, `wait` | the frames that filter accepts, with the 8-byte header; four held, more dropped and counted |
| `send` | `failed` on a listen-only channel and for a probe request. Before a country, the first other send takes the world rules (`00`, 801-DRV-0071). Otherwise `busy`, and that channel is admitted (below) |
| `attach`, `kick` | `invalid`; a ring capability is dropped |
| `set-country` | checked by `country::Limits`: `invalid` when malformed. Otherwise logged as taken, and applied (answered with `info`). Until 801-DRV-0071 an `unknown` platform word made it `failed`; that word is now held back (801 S5) |

**Admission.** The first `send` that may go out admits its channel: an active tune under the station's rule, `grant`, IQK, TSSI and DPK, the station with the filter the station last set, the data mode, then `service.rs`. The service logs `[RTW89] DATAPATH ADMITTED ON CHANNEL n`.
- **A probe request admits nothing** (2026-11-08). A scan visits every channel, and Linux too calibrates a channel when the station joins there (`rtw89_core_sta_add` → `rtw89_chip_rfk_channel`), not on each scan hop (`__rtw89_set_channel`). So scans stay receive-only, and the join's authentication admits the network's channel.
- **Before this,** the scan's first probe on channel 1 admitted channel 1 (2026-11-06). Data mode then held channel 1 alone, and the network, on channel 8, was never heard.

**In data mode** (`service.rs`):
- `set-channel` takes the admitted channel only; others are `invalid`;
- `set-country` keeps the channel when the new limits still let the radio send there at no less EIRP than it was admitted under. It answers `info` and logs `[RTW89] COUNTRY XX IN DATA MODE: CHANNEL n KEPT`.
- **Otherwise, for now, a debug mode** (`KEEP_SENDING` in `service.rs`, the maintainer's decision of 2026-11-08: everything runs first, hardening later). The driver logs `… NOT ALLOWED AT ITS ADMITTED POWER: SENDING SHOULD STOP; DEBUG MODE KEEPS SENDING`, answers `info` as if the limits were applied, and keeps sending at the admitted power. **This breaks wlanhw 1.2's rule** that a radio which cannot apply its limits sends nothing.
- With `KEEP_SENDING` false, it answers `failed` and logs `… NOTHING SENT UNTIL A COUNTRY ALLOWS IT`, and every `send` is `failed` until a country it can keep. The host tests run both modes.

**On the Ryzen, 2026-11-08** (channel 8, CoolNet2):
- **The hardware filter.** The data mode, and the receive-only service once the station sets a network's filter, narrow it to Linux's station value ([station notes](rtl8852be-station.md)). The scan's broad filter let the whole busy channel through: within 200 ms the CMAC reported a receive DMA error, and the firmware's L0 recovery reset the BB and the scheduler (`SER_DBG_INFO`). With the narrow filter, no such recovery was seen.
- **Receive draining.** The data service takes up to 32 descriptors of each ring a step (`DRAIN`); before, it took one.
- **A debug mode, `OUTLAST_SER`** (`data_mode.rs`, on until hardening): a BB in reset or a scheduler off is waited for up to 60 ms. A scheduler still off after a recovery the firmware counted is restored, and the service logs `FIRMWARE RECOVERY n …`.
- **Diagnostics in the stop line:**
  - the registers the first shutdown found, and the same ones at admission (`REGISTERS AT ADMISSION`);
  - when each first changed;
  - for `TransmitTimeout`, what the frame went through (`tx::Seen`).
- **A debug mode, `RPQ_ALONE`** (`tx.rs`, on until hardening). A data queue's frame asks for no firmware report and is final on DMA and the RPQ, one hardware try a submission, as Linux's PCIe path takes a status (`rtw89_pci_release_rpp`). The firmware reported the management queue's frames only: the WPA2 handshake's message 2 was fetched and released, never reported.
- **Path B's TSSI DAC gain table** is written unread, as Linux. On the Ryzen its readback changed in three admissions of four; path B is shared with Bluetooth.
- **With all of it the Ryzen joined CoolNet2:** WPA2 handshake, `JOINED (TRY 1)`, DHCP `192.168.2.169/24` on the Wi-Fi card.
- **Not solved:** the coexistence keeps the `NoLink` policy after association.
  - **What it did:** 6 minutes after joining, during `bt_hci`'s 10-second inquiry, the station heard nothing for 10 s and lost the network.
  - **What Linux does:** it tells the coexistence of the connection (`BTC_ROLE_MSTS_STA_CONN_END`), which then shares the time between Wi-Fi and Bluetooth. That is [213-DRV-0052](../issues/213-DRV-0052-coexistence-after-association.md).
- **A correction to the stop line's changes.** The BB in reset and all host DMA stopped that the changes showed at a stop (`0=D9DC3F80`, `1010=000BFF00`, `+TX`) were the data mode's own shutdown: the service read the registers after it. It no longer watches once the data mode has shut down.

**The Wi-Fi alone, the cable out (the Ryzen, 2026-11-08).** Effector's TLS to hesere.uz went over it. Each step below was found on the machine, and each is a debug mode, on until hardening:
- **`OUTLAST_SER`'s wait has its own time.** A recovery's wait moves the pending frame's deadline, and the operation's, by its length and `SER_GRACE_US` (20 ms). A recovery that begins during the checks is outlasted, and the checks are made once more. Before, a recovery during the first traffic ran out the frame's 100 ms inside the station's register check.
- **`DROP_UNACKED`** (`tx.rs`): a frame the RPQ releases as dropped (lifetime, or a MACID drop during a recovery) is answered as sent and not acknowledged. False: the data mode stops (`Dropped`).
- **`REPORT_EACH`** (`post_mac.rs`): a release report per frame. Linux batches 30, with many frames in flight. With one in flight, each try waited for the batch's timeout, about 10 ms, so wlan's 15 tries outran a send's time.
- **`SEND_TRIES`** (`service.rs`): at most 8 submissions a send, so a frame nobody acknowledges ends in its time as not acknowledged, and wlan's rate steps down. At 24 Mb/s, 12 tries in a row went unacknowledged.
- **A release taken in a step is acted on in that step**, not after the next receive wait.
- **A send times out in the service 10 ms before the data mode's own deadline** (`FRAME_MARGIN_MS`), so a frame that never ends stops the service as `TransmitTimeout`, with `Seen`, its queue, length and age.
- **`OWN_DEADLINES`** (`data_mode.rs`): an operation keeps its own deadline while a frame is pending. Capped by the frame's, the operations near it ran out in whichever register they read: `Session(Permit(Timeout))` at `0x13b0`, `Register(Timeout)`.
- **`RESTART_AFTER_FAILURE`** (`main.rs`): after a failure that stopped, powered off and quiesced the device, the driver exits, and `init` starts a fresh one within its budget (3 in 60 s). `wlan` learns the fresh radio's country only at its own start: asked of NET on `internal-dev-messages`, 2026-11-08.

- **`OUTLAST_SER` beyond the start of an operation:**
  - a recovery that holds the radio off in the coexistence reconcile is outlasted, and the reconcile runs again;
  - one found by the checks or the permits at an operation's start is outlasted, and they run again;
  - the data permit takes a scheduler at zero, as the recovery leaves it, as the next operation's to outlast.
- **`KEEP_LATE`** (`service.rs`): a send that outruns its time is answered as sent and not acknowledged, and its release is awaited 1 s more (`LATE_MS`), while every other send is `busy`. The log says `A FRAME OUTRAN ITS TIME (n): …`. One that outruns that too stops the service.

**Still open:** a frame fetched by the DMA whose release does not come within its time. One frame waited over 45 ms after five tries, about 3 minutes into a run; with the connected station's policy (213-DRV-0052), one after 9 minutes. Without `KEEP_LATE` and `RESTART_AFTER_FAILURE` the machine stays off the network then.

After a stop with `RESTART_AFTER_FAILURE` false, or one that did not quiesce, it answers `no-device` for as long as the process lives (`absent_forever`).

**Its limits:**
- data mode keeps the one channel it admitted. It has no retune or recalibration of another channel, so a station that loses its network there cannot scan elsewhere until the driver restarts;
- a scan's dwell includes the passive tune: the station's timer starts before `set-channel` is answered. The `SCAN` line measures the tunes on the hardware.

**Host tests:** `tests/rtw89_receive_service_cases.rs` and `tests/rtw89_service_cases.rs` in `rtw89_service_host.rs` run the real setup over the initial-PHY fixture and the IPC seam. They cover:
- `info` and `set-channel`;
- `send` before and under a country, a probe request and a listen-only channel;
- countries taken, applied or not, and malformed ones refused;
- a scan's counts;
- a beacon delivered while another station's data is filtered out;
- in data mode, countries kept and refused at the admitted power.

`tests/rtw89_connection_cases.rs` runs the full admission, from the receive-only start to a frame sent, with the join's filter kept.

## An unknown platform word, an unknown country and a stalled receive (801-DRV-0071, 2026-11-14)

The maintainer's decisions of 2026-11-14 (801 S5): a connection that worked is never lost to an unknown power limit, and with the country unknown too the radio sends at 75 % of its maximum.
- **The platform word.** `platform=unknown:<why>`, a malformed or repeated word, or none, maps to `PlatformInput::HeldBack`: no platform limit, the country's limits alone, as `CheckedAbsent`. `PlatformInput::Unknown` itself still refuses. `main.rs` logs `PLATFORM POWER POLICY FROM INIT: UNKNOWN (<why>): HELD BACK, NOT ENFORCED: SENDING UNDER THE COUNTRY'S LIMITS ALONE (801)`.
- **The 75 % cap.** 75 % in mW is 1.249 dB down; the MAC's power registers are in half-dB, so 3 steps (1.5 dB, 70.8 %). Under `00`, `Policy::resolve` holds each NTX limit at the channel's maximum less 3. The maximum is the most any rate is given there: the least of its by-rate value and its section's 20 MHz non-beamformed limit (`channel_power::maximum`). Under the world group it is 29 half-dBm on 1 TX on channels 1 to 11 (28 on 12 and 13) and 23 on 2 TX (21 on 13), so the limits become 26 and 20. A known country is not capped. TSSI's 12 dBm floor still holds.
- **No country.** A station that sends anything but a probe request before its first `set-country` gets `country::Limits::world`: the world rules at their own EIRP, so the channel is admitted under `00` and capped.
- **In data mode,** a `00` that arrives keeps the power the channel was admitted at. The cap is set at admission.
- **A stalled receive** (`service.rs`'s `QUIET_MS`). No Wi-Fi frame for 10 s while the filter lets a network in (a scan's filter or a BSSID) stops the data service as `Fatal::ReceiveStalled`, with the line `RECEIVE STALLED ON CHANNEL n: NO FRAME FOR q MS …`. The driver then stops the device and exits (`RESTART_AFTER_FAILURE`), and `init` restarts it. A filter with no network arms nothing.
- **Unacknowledged sends** (`UNHEARD`). 16 unicast sends in a row not acknowledged, over at least 5 s, are logged once a run; the service goes on.

