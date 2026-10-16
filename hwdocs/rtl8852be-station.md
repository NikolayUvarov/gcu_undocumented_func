# RTL8852BE initial station context

`station::prepare` is a private continuation after the actual initial channel
calibration coordinator. It binds one MACID, port, address CAM and BSSID CAM
(index zero) to the same PHY attempt, command/RX session, selected channel,
policy version and actual board MAC. It accepts the real calibration result,
BTC owner and firmware PHY capabilities. It creates the source initial
`STATION` / `SELF_CLIENT` / `NO_LINK` context, with an all-zero initial BSSID;
it does not invent an association, security keys or permission to transmit.

The source is Linux rtw89 at commit
[`05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`](https://github.com/torvalds/linux/tree/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89),
under its BSD-3-Clause alternative. The exact dispatch is
[`mac.c:4825`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/mac.c#L4825):
port update, initial DMAC/CMAC tables, MACID unpause, role creation,
disconnected join information, CAM, then default CCTL. The 8852B's default
DMAC command and security-table hooks are absent, so no extra commands are
invented. Initial role values come from `core.c:4649–4700`; port programming
comes from `mac.c:4265–4787`, CAM packing from `cam.c:646–948` and the matching
`cam.h` masks. `fw.c:3186–3234` defines the default CCTL, including
`MGQ_RPT_EN`, actual firmware antenna selection and source-zero power mode,
Doppler and power tolerance.

The indirect table window ends at BAR offset `0x40020`; this stage refuses a
shorter grant before touching it. The observed Ryzen BAR2 is 1 MiB, and init
passes the whole reported BAR length. DCTL's four words and CCTL's eight words
are initialized through the source selector at `0xc04` and aperture at
`0x40000`. Initial writes are read back immediately. After firmware ROLE/JOIN,
only the CCTL fields explicitly updated by this command are compared:
unrelated control fields belong to the firmware and need not remain equal to
the host's initial table. The original selector is restored on success and
attempted independently on failure. CAM completion uses the actual firmware
done ACK; no unsupported physical CAM address or fabricated readback is used.

All five commands use the existing exclusive Session H2C sequence and RX/ACK
owner. MACID unpause requests receipt, as in the source; role, join, CAM and
CCTL request done ACKs. The normal periodic receive-ACK policy remains intact.
The final MACID pause, port state, report path and masked CCTL fields are read
back, followed by actual BTC ownership and PHY/RX validation. Ordinary DMA
TX, the packet scheduler, beacon transmission and forced beacon transmission
remain stopped. Port function enable is not packet-TX authorization.

A common 600 ms work deadline covers register access, command consumption,
required ACKs and final checks. Modified failure independently stops BB/RF,
marks the PHY failed, disables port/beacon/forced transmission, and restores
the selector. Cleanup errors are retained separately and DMA remains owned.
An uncertain command is never retried. A second initial creation over an
active port is rejected. Association, security, live TX queue activation and
the WLAN service adapter remain later steps; this token alone cannot establish
those capabilities. The initial filter continuation below adds local/BSSID
address filtering, without an associated join.

The registered station harness extends the same cold device used by the
initial PHY and channel coordinator. It performs real production OTP,
firmware transfer, PHY-cap query, MAC/RX setup, reusable initial PHY startup,
channel selection and IQK/TSSI/DPK before station setup. Indirect SRAM and
firmware command application are modeled from actual consumed DMA payloads;
ACKs still traverse the production RX descriptors and Session parser. It
never reseeds production guard registers between stages. Calibration policy
is test-only, and modeled register/firmware responses are not device evidence.
Eleven new cases compile; execution follows source build and raw publication.
They cover exact command masks/payloads and ordinary-TX stop, short BAR,
changed policy, repeated creation, legitimate firmware-owned fields, ignored
CCTL despite done ACK, stalled role consumption, ignored/applied-failed port
writes, the absolute deadline and selector restoration failure.

## Source hashes

| Source | SHA-256 |
|---|---|
| `mac.c` | `ce5c09696ba6dcdc2003ce28803052fd3238c9da55188046e3e6e1ed82658055` |
| `fw.c` | `64c86436e8e0f7f9ecd4364d2d2c48be4549a0448b3ce738eee7e8c25e5875ce` |
| `fw.h` | `bae6675fc95c3f6edd348aaa78c2bbb5cc7768f1961a91fb95ff49624aa9e345` |
| `cam.c` | `e8c2f9c30bf48cfa99366cc75cc89f1a9c0beec8d014a466eeadb0951fb1e381` |
| `cam.h` | `70ba40d1c1de6603162f8ebff009e3b5b28fa54aa8d4bf923282e6f1ee9cd93e` |
| `mac80211.c` | `c0206be51ff92f4adc9a9192627aa3663d9087cc96fcb608dfd3b16f38865108` |

No proprietary firmware bytes or private hardware identifiers are included.

## Published execution

After source `08b9faeb` was published in `5aebc778`, the exact saved station
host binary passed all eleven new cold continuation/fault cases. The complete
registered harness passed 212 cases, failed none, and retained one existing
private-firmware case as ignored, in 0.61 s. Native release compilation passed
in 1.44 s; the inherited unused continuation-field warning does not affect
this model evidence. The log is `/tmp/mind-station-results-08b9faeb.log`.
No physical station setup, packet reception or transmission is established by
these host-model results.

## Revalidation before data activation

`Prepared::validate` checks the same board, channel/policy, PHY attempt,
firmware session and actual BTC/RX owner again under a caller limit of at most
10 ms. The original stopped-TX PHY guard is unchanged. It rechecks port,
MACID pause, report routing and the exact written CCTL masks, restoring the
indirect selector. Completion after the common deadline is rejected; no time
bound promises progress under indefinite preemption. A failed live check
marks the PHY failed and independently stops radio/port. Invalid identity or
policy is refused before these hardware checks.

The separate `check_registers` helper performs fixed context reads and scoped
selector changes for a later owner that already holds an actual active-mode
permission. It does not validate DMA/session ownership or grant activation.
Its caller supplies the bounded register window; a primary check failure and
selector restoration failure remain distinct. A data-mode owner must consume
the actual station token and retain its own policy/queue/receiver identity.
Four additional cold continuation cases compile, covering current context,
changed CCTL/report routing, changed policy, and owner/final-selector deadline
failure. Execution follows this continuation's own raw publication.

After `7eb1117d` was published in `3881b629`, the exact saved validation
harness passed all 15 station cases (the original eleven and four new cases).
The complete harness passed 218 cases, failed none and retained one existing
private-firmware case as ignored, in 0.62 s. The log is
`/tmp/mind-station-validation-results-7eb1117d.log`.

## Initial receive filter

`Prepared::configure_filter` is an initial, stopped-TX continuation under the
same 600 ms work limit. It validates the actual station, PHY, channel/policy
and BTC/RX owner, updates address/BSSID CAM through a real done-acknowledged
command, writes the receive filter, checks it and the station context, then
revalidates the live owner. Only that success installs `current_filter()`.
Ignored, failed or late register writes and uncertain CAM commands stop the
radio/port and fail the PHY; they do not publish a filter or retry the command.
The initial MACID role remains NO_LINK; this is not an associated join.

CAM uses the source SMA hash/local address and TMA hash/BSSID fields, including
the BSSID CAM bytes (`cam.c:764–885`). A zero requested own address resolves to
the actual radio MAC. The physical `Prepared::mac()` identity remains the
board MAC; a future frame sender must use the configured filter's `own()`
when validating an explicit local-address override.

The hardware register uses the source combination ALLMULTI, OTHER_BSS,
BCN_PRBRESP_PROMISC and PROBE_REQ (`mac80211.c:304–378`), with FCS failures
disabled and no sniffer mode. The resulting configuration is `0x03004400` at
`0xce20`, preserving the MPDU-length field exactly as `mac.c:2546` requires.
`check_registers` checks this configuration once a filter is installed. This
broad hardware path is narrowed by the immutable software predicate, matching
the existing `bcm_wifi::radio::wanted` and `wlanhw@1.1`: scan beacons and probe
responses, own-addressed management/data, and broadcast/multicast from the
selected BSSID. Group ToDS/WDS, unrelated BSS and control/reserved frames are
rejected. The predicate receives an already structurally validated MPDU
without FCS; descriptor errors and variable headers belong to `frames`.

The pure `Filter` value is address policy, not hardware authority. Data-mode
activation must consume a real `Prepared` whose `current_filter()` is present
and revalidate its registers. RX already runs during earlier initialization,
so initial exposure must also drain/drop or accurately tag pre-configuration
records in both the inbox and hardware rings; the station token does not
attest that these are empty. A second initial configuration is refused;
future runtime updates must drain or tag the old receive epoch before changing
the scan flag/filter, including records retained during firmware ACK waits.
The single RX owner must dispatch/filter ordinary traffic during those waits;
the finite inbox does not promise to absorb arbitrary channel traffic.

Seven new cold continuation cases and seven pure policy cases compile; they
have not run before this slice's raw publication. They cover source CAM bytes,
preserved MPDU limit, required CAM ACK/DMA completion, changed policy/filter,
ignored/applied-failed/late writes, forbidden initial reconfiguration, own
override, group-address direction rules, scan selection and malformed input.
No hardware RX, association or packet-TX evidence is claimed.

After source `29175647` and its epoch clarification `2919a3c3` were published
in `bc802db7`, the exact saved binaries passed all seven new cold filter cases
and seven pure policy cases. The station harness passed all 22 station cases;
its complete run passed 225 cases, failed none and retained one existing
private-firmware case as ignored, in 7.02 s. The pure filter run passed 7/7 in
0.00 s. Logs are `/tmp/mind-station-filter-cold-results-29175647.log` and
`/tmp/mind-station-filter-results-29175647.log`. Native release compilation
passed in 1.56 s. These are host-model observations on the source branch,
not tests of `fast-test` or evidence of physical receive/transmit operation.
