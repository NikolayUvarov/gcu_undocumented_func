# Network performance — `x86-64/QEMU-0`

The network stack benchmark of issue 106 and what it decided. Appendix B.2: performance features are taken on the evidence of a measurement on a profile, not by default.

## Bounded service work (550-NET-0031/0032/0033, 2026-10-15)

The per-card two-second deadline from 550-NET-0031 allowed three slow cards
to accumulate six seconds in one poll; polling both before and after a client
could postpone the next receive by twelve seconds. The 550-NET-0032 change
uses one 250 ms deadline for all driver calls in a stack poll and one poll
before each client receive. Calls after expiry are skipped. A persistent
round-robin cursor resumes at the next card after a budget-consuming card.
Link checks run before ingress so a busy queue cannot starve them. Pending
socket replies, timers and cleanup still run after the card loop yields.

With three present cards, two budget-consuming peers impose at most 500 ms
of driver waits before the first visit to an eligible healthy last card. Each pass
offers client handling after at most 250 ms of requested driver waiting.
There may also be the 10 ms client receive wait between passes, bounded CPU
work and kernel scheduling/timeout overhead; this is not a real-time bound.
The stack still processes at most 64 incoming packets per visited card,
followed by an egress pass if time remains. A pending frame waits until the
next loop's poll; there is no second full poll between client receives.

A separate per-card rolling work quota preserves 550-NET-0030's
second limit independently of liveness: no fresh call is admitted beyond
2000 ms of measured visit work retained from the past five seconds. Eleven
500 ms completion buckets per card conservatively retain the entire duration
of a visit until its bucket is fully five seconds old (retention is 5–5.5 s
from completion). Work spanning a bucket boundary is charged entirely to its
completion bucket. There is no fixed-window reset or idle credit. The next
IPC gets at most the remainder of both this quota and the global turn.
Backwards/wrapped monotonic time fails closed instead of renewing allowance.

The accounting uses elapsed whole milliseconds for complete visits. CPU work
is also charged at that resolution; submillisecond quantization and a final
operation's kernel scheduling/timeout or CPU overrun are outside the bound.
This is a bound on admitted waits and measured work, not a hard real-time
or CPU-time guarantee.

A short-budget timeout does not erase link state or label a card dead.
Quota exhaustion yields until old work charges expire; no new five-second
cooldown starts from a timeout. The WLAN
pump can spend up to two seconds in radio/key-store calls: a live WLAN that
becomes ready after exhausting its allowance can wait until older charges
expire (the tests exercise readiness after two seconds and retry at 5.5 s). Other cards and clients continue meanwhile. Quick raw-frame
replies are expected normally, but this expectation has not been measured
on the MacBook. This policy does not establish a heartbeat latency bound.

The existing four production deadline/ingress host tests remain, with fourteen
new production scheduling regressions for silent peers, slow successful
peers, remaining time, recovery, idle slots, continuous ingress, rolling
work quotas, fixed-window boundary bursts, cheap healthy peers, clock reversal
and a live WLAN completing its long operation (`tests/network_budget/Cargo.toml`).
Implementation `b5b9b3ddb946b6c2aa223f6dcb7ca6ddf979ab34` built for
`x86_64-unknown-none`. After its publication in `fast-test` `0b5bd1bf`, all
18 host tests passed on the own branch on 2026-10-15 (zero failures/ignored;
x86_64 Linux 6.6.87.2 WSL2, rustc 1.101.0-nightly c36f14571). Nothing was
built or tested on `fast-test`. Integrated network QEMU checks and hardware
latency/loss measurements remain pending for this configuration. The supplied
MacBook log does not establish the cause of the reported ping loss under
load. No Effector heartbeat or physical NIC result follows from these host
checks.

The separate 550-NET-0033 change dispatches an accepted WLAN client before
background receives. Each receive batch starts at most eight frames and
checks a 100 ms cooperative slice before starting another frame. A frame
already received completes its station processing and actions using the
original two-second deadline shared with the client handler. The slice does
not shorten radio/control/key IPC deadlines. If the aggregate deadline has
expired, the loop does not start background processing or a new station tick.
Protocol handshake and link-loss timers are unchanged.

With a continuous stream of 10 ms receives and no other work, a new client
gets a dispatch opportunity after eight receives (80 ms), instead of waiting
behind a 64-frame batch. This is the model for the added production-helper
tests, not a measured hardware limit. Implementation `31af1f01` built for
`x86_64-unknown-none`; after publication in `fast-test` `a98052b5`, all 26
network-budget host tests and all 12 unchanged `dot11` station tests passed
on the own branch on 2026-10-15 (zero failures/ignored, Linux 6.6.87.2 WSL2,
rustc 1.101.0-nightly c36f14571). No tests ran on `fast-test`.
A single synchronous call or the final frame's actions can still exceed
100 ms and the caller's 250 ms budget. Existing action losses when the
aggregate two-second deadline expires during frame actions are not repaired
by this change. Integrated WLAN QEMU and MacBook measurements remain pending.

## How it is measured

`netbench <a.b.c.d>:<port> [MiB]` is a console program. It gets a flow grant from the policy broker and a `sysmon` client. It runs against the host's bench server (`tests/qemu_smoke.py`, `netbench` suite) in these phases:

- **DOWN:** the host sends *n* MiB over TCP.
- **UP:** the guest sends *n* MiB; the host answers once it has everything.
- **UDP:** 500 sequential round trips of 64 bytes to a UDP echo.
- **CHECKSUM:** the stack's software TCP/UDP checksum loop over 1 MiB, timed in the guest.

Each phase reports its rate and the CPU time that `netstack` and the card drivers spent during it, read from the kernel's task records through `sysmon`. Those times are per MiB, or per round trip for UDP. The benchmark program's own time is not counted.

```
python3 tests/qemu_smoke.py --suites netbench [--bench-mib 4] [--bench-runs 2]       # QEMU user networking
python3 tests/qemu_smoke.py --suites netbench --tap mindtap0 --bench-runs 2           # tap backend
```

With QEMU's user networking (slirp) the backend has no virtio-net header, so QEMU withdraws every offload feature: the card offers neither checksum nor segmentation offload. With a tap backend (`vnet_hdr=on`; the host side of the tap at 10.0.2.2/24, the guest on its static fallback 10.0.2.15) the card offers `VIRTIO_NET_F_CSUM`. CI runs both (`.github/workflows/ci.yml`, job "network benchmark").

## Results with one call per frame (issue 106; 4 MiB each way, 2026-10-04)

QEMU 8.2 with TCG (no KVM), 4 vCPUs, on a 4-core Xeon container host. The modern VirtIO interface with MSI-X and a tap backend; two runs per setting. Rates vary by about ±15 % between runs on this host; the CPU time per MiB is steadier.

| Phase | Offload off | Offload on |
|---|---|---|
| TCP download | 1.88 MiB/s; `netstack` 299 ms/MiB, drivers 231 ms/MiB | 1.89 MiB/s; `netstack` 296 ms/MiB, drivers 229 ms/MiB |
| TCP upload | 1.61–1.65 MiB/s; `netstack` 334 ms/MiB, drivers 270–288 ms/MiB | 1.66–1.69 MiB/s; `netstack` 319–325 ms/MiB, drivers 256–267 ms/MiB |
| UDP 64-byte round trips | 417–425 per s; `netstack` 1.3 ms, drivers 0.8 ms per round trip | 403–425 per s; the same |
| Software checksum | 1.0–1.1 ms per MiB | (not used for sent TCP/UDP) |

User networking, the default in tests, gives the same rates and CPU times as the "offload off" column.

## What it shows

- **Checksums are not the bottleneck.** Computing them in software costs about 1 ms of CPU per MiB. The stack spends about 300 ms per MiB, so checksums are about 0.3 % of the stack's work. Transmit checksum offload saved about 3–4 % of the stack's CPU on upload, which is within the noise of this setup. Receive checksums cannot be offloaded with user networking, and with tap the possible saving is of the same size.
- **The cost is per frame, not per byte.** A MiB downloaded is about 720 data frames in and 360 acknowledgements out. That makes about 275 µs of `netstack` time and about 210 µs of driver time per frame. Each frame is one MIND IDL buffer call between stack and driver: a memory capability made, mapped, copied and revoked. A UDP round trip (two frames and two client calls) costs about 2 ms of CPU together.
- **Segmentation offload (TSO/GSO, mergeable receive buffers)** would put fewer, larger frames through the same per-frame path. It needs frames larger than the 1514 bytes of `idl/net.wit`, and smoltcp does not build TCP segments larger than the MTU. It is not done. The per-frame call is what to make cheaper first, with frames batched in shared rings between stack and driver ([issue 107](../../issues-done/107-batched-frame-path.done), below).

## The frame ring (issue 107)

Counting the stack's calls to the driver during a download explained the cost per frame. Each received frame took about 1.9 `receive` calls (almost half of them found nothing) and one `send` call for its acknowledgement: about three buffer calls per frame, each a memory capability made, mapped, copied and revoked.

Since `net.wit` 1.2 the stack lends the driver a 132 KiB region once (`attach`). The region holds a transmit and a receive ring of 32 slots each (`mind::netring`):
- Received frames reach the stack without any call.
- Frames to send cost a slot and, at most once per poll, a `kick` word call.
- The old calls stay for the shell's diagnostics. The driver goes back to them when the stack's task ends.

Same setup as above (tap, two runs per setting):

| Phase | One call per frame (106) | Frame ring (107) |
|---|---|---|
| TCP download | 1.88 MiB/s; `netstack` 299 ms/MiB, drivers 231 ms/MiB | 3.77–3.90 MiB/s; `netstack` 118–123 ms/MiB, drivers 104–107 ms/MiB |
| TCP upload | 1.61–1.65 MiB/s; `netstack` 334 ms/MiB, drivers 270–288 ms/MiB | 2.94–2.97 MiB/s; `netstack` 161–174 ms/MiB, drivers 137–148 ms/MiB |
| UDP 64-byte round trips | 417–425 per s; `netstack` 1.3 ms, drivers 0.8 ms each | 603–648 per s; `netstack` 0.83 ms, drivers 0.34 ms each |

The rates doubled and the stack's and the drivers' CPU time per MiB fell by about 55–60 %. With checksum offload on, the result is the same within the noise. Checksums remain about 1 % of the stack's remaining work.

## Decision

- **Transmit checksum offload** is implemented: `net.wit` 1.1 adds `offloads` and `send-partial`, `socket.wit` 2.2 adds `offload`, and the shell has `ip offload on|off`. `netstack` writes the pseudo-header sum and the card completes the checksum. Without offload the stack computes TCP and UDP checksums itself. The setting is **off** by default, because no gain beyond the noise was measured. Both settings are tested: the host's TCP stack drops segments with a wrong checksum, so the upload phase checks them.
- **Trust:**
  - With transmit offload, the card computes the checksums of what is sent. It already sees and could change those bytes: every DMA-capable driver and its device is in the TCB for memory isolation (MC-1.5, [tcb.md](tcb.md)). Offload adds no new trust.
  - Receive checksums are always verified by the stack. The card is not trusted to vouch for the integrity of received data, and `VIRTIO_NET_F_GUEST_CSUM` is not negotiated.
- **Segmentation offloads** are not used. Frames still cost about 110 µs of stack time each, and much of that is the client calls (`tcp-receive` moves 4 KiB per call) and smoltcp itself. Larger frames would need a frame size beyond 1514 bytes in the ring and TCP segmentation that smoltcp does not do. Decide again with a measurement on hardware with KVM.
- **The frame ring** is used whenever the driver offers it (`net.wit` 1.2).

## RTL8852BE initial board access (213-DRV-0032)

The native `rtw89_wifi` service provides a bounded initial MAC power/XTAL/EFUSE
read cycle for digital cut 1, under KRN-0114's MEM-only BAR2 grant. It requires
known inactive MAC state and a calibrated high-resolution clock, holds the
baseband in reset, attempts independent OTP/power cleanup, and rejects
unsupported RFE or missing identity data. Its TCB includes the PCI/MMIO grant,
uncached mapping, calibrated kernel clock and the pinned BSD register/power
sequence documented in [the hardware notes](../../hwdocs/rtl8852be.md).

Evidence: own branch `codex/213-DRV-0032-rtl8852be`, `a6e69a07`, x86-64
release build and 16 host state-machine/fault-injection tests passed on
2026-10-15, with tests run after `fast-test` publication `0b5bd1bf`. The
mock hardware checks bounds, state transitions and cleanup; it does not
establish device timing or electrical behavior. KRN integration/full gate and
actual Ryzen MAC/board-read/shutdown evidence remain pending. No host DMA,
firmware download, BB/RF calibration, WLAN registration, association, IP
traffic or Effector connectivity is implemented or claimed by this slice.

The separate channel-12 command queue introduced a bounded descriptor/payload
ownership implementation, initially without a native caller. Its TCB
requires x86-64 coherent WB DMA, UC MMIO, exact PCI/MMIO identity and externally
verified quiescence before region release. It does not grant DMA, enable
mastering or supply MAC/PCIe startup, firmware policy or radio operation.
Evidence for this additional slice: own-branch `f6caafca` native x86-64 release
build passed; all 14 focused host tests passed on 2026-10-15 at 21:01 UTC
(0.03 seconds), after `fast-test` publication `dd735d0a`. The tests use a
simulated coherent DMA device; no hardware DMA evidence or portable
architecture guarantee is claimed. See the hardware notes for the
eight-completed-command retention and fault contract. The frozen `a99cc44a` gate
does not include this later change and cannot be used as its full-gate evidence.

The later MAC download-memory preparation implements the pinned RTL8852B
DLFW WDE/PLE quotas and HFC command-channel configuration under active-MAC,
stopped-BB/WCPU and PCI MEM-only guards. It bounds readiness waits and retains
independent HFC/DLE cleanup errors. Own-branch `371d41db` native x86-64 release
build and 11 focused host tests passed on 2026-10-15 UTC, with tests after
`fast-test` publication `60a100be`; the hardware notes retain the build
invocation correction and precise scope. That library slice supplied no
native host DMA or radio startup; actual hardware operation and the subsequent PCIe/firmware stages
remain unvalidated.

The later PCIe preparation implements the pinned RTL8852B MDIO/DBI path,
temporary L1 restoration, channel stops/index/mode setup and BDRAM reset
after command-ring installation. It checks the MAC download profile and
preserves PCI MEM-only/HCI-off state. Primary and independent restoration/
stop failures remain explicit, and no DMA map is freed. Own-branch `4feee434`
native x86-64 release build and all 16 focused host fixtures passed on
2026-10-15 UTC after `fast-test` publication `1dee0b3f`, before native
integration. No hardware DMA, firmware download or radio
operation is established, and the frozen earlier `a99cc44a` gate does not
cover this additional slice.

The separate WCPU downloader requires the exact full private file SHA-256,
cut-1/RFE-1 metadata and actually read nonsecure OTP pair before device writes.
Its TCB includes the existing SHA-256 implementation, trusted board-map origin
and the command queue's coherent DMA/kernel-quiescence contract. It bounds
reset/start, header/section transfer and status polling and retains failed
DMA ownership with separate cleanup errors. This is pinned-file integrity,
not vendor-signature verification. Own-branch `cf768d22` native build passed;
after `fast-test` publication `45387d45`, all 18 synthetic downloader cases,
the separately invoked private-file hash/mutation fixture and 14 FWCMD
regressions passed on 2026-10-15 UTC. These establish model behavior and
identity of the private file, not hardware operation. Native integration is described next;
actual Ryzen firmware/radio operation remains pending.

The 213-KRN-0114 continuation wires a single native firmware diagnostic into
that executable. It verifies the private file before board MMIO, admits the
actual board/OTP policy before queue initialization, and keeps a 128 KiB
coherent WB DMA mapping through all outcomes. MAC/PCIe/ring setup precedes
kernel mastering; only the command channel and TXHCI are enabled. Independent
DMA/WCPU stop, kernel-verified MEM-only state, fresh-budget power-off and a
final kernel stop follow every modified attempt. A failed MEM-only check
prevents further MMIO. No cleanup result permits memory reuse; there is no
radio endpoint or retry. Startup-model tests are not hardware or IOMMU
containment evidence, and native Ryzen firmware/DMA operation remains pending.
The integration branch `5f1d940d` builds `rtw89_wifi` and `init` for x86-64.
After raw publication `649fea37`, all ten startup/activation host cases
passed on the own branch (2026-10-16, zero failed/ignored, 0.00 s). These are
orchestration/register tests. The separate cumulative model in `09d1ceaa`
then passed two standard host cases and three explicit local private-file
scenarios after publication `8461fd20` (2026-10-16, x86-64 WSL2,
rustc 1.101.0-nightly c36f14571). It carries one modeled device through
board/power/MAC/PCIe/command-queue/download/stop stages, compares every section
byte with the pinned file, and checks missing-readiness cleanup. Readiness,
OTP, PCI transitions and synchronous DMA consumption are simulated; actual
Ryzen firmware/DMA operation and hardware timing remain unvalidated.

The later `runtime_mac` library adds bounded source-derived SCC DLE/HFC,
station/MPDU/security and CMAC0 preparation after verified firmware readiness.
Firmware-mediated scheduler pause precedes chip BB/RF enable; ordinary host
DMA remains stopped. An independently bounded cleanup retains errors and
DMA. Own-branch `02b6b4e3` native build passed (0.43 seconds, no warnings);
after `fast-test` publication `8461fd20`, all 14 focused runtime-MAC host
fixtures passed (0.12 seconds), on 2026-10-15 UTC. Hardware validation and
a full gate containing this slice remain pending. It is not called by the
native service and does not establish full runtime MAC, calibrated RF or
WLAN service operation; see [exact scope](../../hwdocs/rtl8852be.md#single-band-runtime-mac-preparation-library).

The later RXQ/RPQ library retains exact RTL8852BE descriptor/chip settings,
copies each bounded packet before releasing its descriptor, and parses AX
RX/RPQ/C2H metadata without per-packet allocation. It targets a separate
future retained 8 MiB DMA layout; the native firmware-only diagnostic grant
is unchanged. Own-branch `b112406d` native build passed (0.47 seconds, no
warnings); after raw publication `cfc4515b`, all 18 focused host fixtures
passed on 2026-10-16 (0.13 seconds, none ignored). Hardware validation and a
full gate containing this slice remain pending. It provides neither RX activation nor
interrupt/WLAN integration. Warm-state ownership/quiescence is still a
precondition, not established by PCI MEM-only decoding.

The initial normal-H2C library adds bounded eight-slot correlation of typed
receipt/DONE events, absolute command deadlines, failure poisoning and a
256-command no-wrap limit for startup work. It does not establish a
continuous runtime firmware transport. Its production adapter reuses the
existing DMA command queue and therefore inherits that queue's TCB and
retained-memory preconditions. The BT RFK prerequisite only observes actual
RUN/REQ scoreboard bits under a bounded read-only wait; that snapshot does
not authorize calibration. Actual BTC state, WLAN scoreboard ownership,
antenna/grant and TDMA policy remain required for a DACK START/STOP callback.
Own-branch `d951d101` native build and thirteen H2C plus six BT-observation
fixtures passed after raw publication `d9266c7b` (2026-10-16, x86-64 WSL2,
rustc 1.101.0-nightly c36f14571; zero failures/ignored). These include a
modeled DMA/C2H path, not actual firmware execution. Native integration and
hardware validation remain pending for this slice.

The subsequent MAC-tail/PCIe post-init library validates installed receive
rings before enabling RXHCI and RXQ/RPQ. All uninstalled data-TX channels,
WPDMA and host interrupt masks remain stopped/masked. The 10 ms stage and
its independent cleanup retain DMA after failures. Own-branch `06a0b932`
native build passed (0.66 seconds, no warnings); after raw publication
`b50f52c0`, nine focused post-init cases passed (0.81 seconds) plus all 18
affected RX cases (0.12 seconds), none ignored, on 2026-10-16 MSK. The
shared zero-only scheduler helper `b947aa40` also passed its build (0.46
seconds) and 17 runtime-MAC cases (0.12 seconds) after publication
`d9266c7b`. Full-gate and hardware evidence remain pending. Its `Receiving` result is not H2C/BTC/PHY or WLAN readiness,
and the native firmware-only service still does not call this library.


The hardware-only `btc_init` library requires the actual RadioLoaded PHY
phase and the unchanged stopped-data-path guard before the source-derived
RTL8852B/RFE-1 PTA/LTE/RF-LUT setup. It has a 200 ms attempt deadline and
independent LUT-close/chip-disable cleanup; modified failures poison the
PHY attempt. Its opaque result is not an RFK grant or complete coexistence
initialization. Actual policy, scoreboard ownership, antenna/grant state and
native integration remain absent from this slice. Own-branch `e6ed1e2c` native build passed (3.53 seconds). After raw
publication `c2facb84`, all nine new fixtures passed on 2026-10-16; the
executable also passed 18 inherited downloader cases and retained that
module's existing optional private-file ignored case (27 passed, zero failed,
one inherited ignored, 0.19 seconds, x86-64 WSL2). No hardware
or calibrated-radio guarantee is added; [scope and source](../../hwdocs/rtl8852be.md#hardware-coexistence-initialization-before-nctl).

The later receive-session validation binds that result to both exact DMA
objects and fresh non-wrapping process-local session numbers. It returns a
borrowed opaque permit only after bounded live register/index checks, with
no MMIO writes. This prevents old tokens accepting reconstructed pools at
the same addresses; it still trusts the adapter's same-function capability
contract. Own-branch `b8f41cb5` built without warnings in 0.51 seconds.
After raw publication `767c2cb8`, all 16 post-init/session cases passed
(0.82 seconds), together with 18 RX cases (0.12 seconds) and 14 FWCMD
cases (reported 0.00 seconds), none ignored, on 2026-10-16 MSK. Full-gate
and hardware evidence remain pending. PHY/native integration does not
yet consume this permit.

The subsequent native MAC/RX continuation supersedes the earlier firmware-only
entry: init grants retained 8 MiB DMA, with disjoint command/RX reservations,
RX installation before mastering, then runtime MAC and receive-only startup
after verified firmware. Independent receive/radio/command stop follows every
modified attempt. Before power-off, a bounded idle-XTAL check prevents cleanup
from replacing an uncertain command. No WLAN endpoint, calibrated channel,
association or network link is claimed. The full x86 image builds; twelve
startup fixtures compile and await publication before execution. Earlier gate
`ac1854f2` does not cover this continuation; physical Ryzen operation and a
cumulative runtime-MAC/RX model remain unvalidated.

The subsequent firmware-ready token correction binds `Ready` to the
successful command queue session and checks that identity before runtime
MAC preparation accesses the device. Existing live guards still reject a
stopped queue. Own-branch `383b27e5` built without warnings in 3.31 seconds.
After publication `badae1bc`, 19 runtime MAC cases passed (0.12 seconds),
18 synthetic downloader cases passed (0.19 seconds), and the explicit
private-file hash/mutation case passed (0.06 seconds). The cumulative
private-file GEN1/GEN2 and missing-H2C model scenarios also passed, on
2026-10-16 MSK. Full-gate and hardware evidence remain pending. This does
not bind the separate future BTC/PHY attempt.

The following `btc_codec`/`btc_control` prerequisite slice adds only exact
profile-105 wire encoding and bounded low-level no-link coexistence register
steps. It distinguishes BT scoreboard reads from the WLAN write shadow,
refuses pending-LTE overwrite, and retains separate source ACK choices.
Own-branch `94762996` built without warnings (0.80 seconds); after raw
publication `1006c136`, its seven codec and eleven control fixtures passed
on 2026-10-16 MSK (0.00 seconds each, zero failed or ignored, x86-64 WSL2).
The complete session-bound owner and applied policy/C2H
orchestration remain unimplemented, and no Initialized or RFK-grant token
is available from this slice. No native or hardware guarantee changes;
[scope](../../hwdocs/rtl8852be.md#exact-coexistence-command-codecs-and-bounded-register-transitions).
Own-branch `b0987183` was published in `fast-test` `7e280d0c` before
executing its compiled startup harness. All twelve cases passed on 2026-10-16
(0.00 seconds, zero failed/ignored, x86-64 WSL2, rustc 1.101.0-nightly
c36f14571), including pending-XTAL refusal, disjoint retained DMA bounds,
all thirteen startup-stage failure positions and independent cleanup.
The later own-branch integration `f314d00c` also builds the complete x86
image with the queue-session and typed PHY/RX guards. These checks do not
establish cumulative runtime device behavior or physical connectivity.

The private MAC report-tail engine requires a real receive permit and live
post-DACK register state before programming PPDU forwarding and RTS fields.
It remains inaccessible to native startup until the complete PHY/RFK/trim
and path prerequisites are implemented. A 10 ms stage and independent
report/RX/radio/command cleanup retain DMA on failure. Own `150f5b80` built
without warnings in 0.71 seconds. After raw publication `1006c136`, seven
tail cases passed (0.27 seconds) and 16 affected post/session cases passed
(0.88 seconds), none ignored, on 2026-10-16 MSK. Full-gate/native/hardware
validation remains pending.


The PHY-capability query shares the scheduler's register mailbox and counters,
requires an actual downloader token for the same live DLFW queue, and only
returns a capability token after a matching complete firmware response within
the total deadline. Raw fields and explicit successful-zero-NSS fallback flags
remain available; OTP data or a missing reply cannot replace the response.
Session identity, independent command/WCPU cleanup and retained DMA remain
part of the contract. Final own tip `a877030f` built without warnings in
0.73 seconds; after raw publication `1006c136`, 10 query cases passed in
0.14 seconds and 19 affected runtime-MAC cases passed in 0.13 seconds,
none ignored, on 2026-10-16 MSK. Native integration and physical-device
evidence remain pending; synthetic responses do not establish hardware
capabilities on the maintainer's device.

The pure BTC profile-105 decoder validates a complete bounded C2H report
stream before yielding typed TDMA/slot/BT-version records, preserves all
policy fields and explicitly surfaces BTINFO, scoreboard, overflow and
unsupported events. It carries no session correlation or RFK permission;
the future owner must process state changes and verify policy application.
Seven host parser fixtures compile; build, execution and native/hardware
evidence are pending for this slice.

The owned startup `btc_session` adapter consumes the downloader token and uses
actual command/RX/Receiving objects for normal H2C and PHY RF-page sends. It
retains ordered bounded BTC events and rejects new stages after an uncertain
failure; raw cleanup remains possible. Thirteen model fixtures compile, with
execution pending raw publication. These seeded-handoff fixtures do not establish
cumulative cold-device behavior, complete BTC initialization or RFK permission.
The native service still does not invoke this adapter or expose a radio endpoint.

Own-branch session commit `3ee6dd54` built for x86-64 without warnings in
1.09 seconds. After raw publication `539b588e`, all 13 session fixtures passed
in 0.08 seconds on 2026-10-16 MSK (zero failed/ignored, x86-64 WSL2).
The two existing runtime-MAC identity tests using the changed downloader helper
also passed separately (0.00 seconds each). Other inherited fixtures were
filtered from this focused run. Independent review found no remaining blocker
in this slice. These seeded-handoff models do not establish cumulative device
behavior, complete BTC initialization, RFK permission or hardware connectivity.

The later native PHY-capability continuation invokes the actual GET_FEATURE
query after successful WCPU download and before runtime MAC. It retains the
queue-bound reply and logs raw words and explicit successful-zero-NSS defaults.
Mailbox failure ends the diagnostic with independent cleanup; this adds no
association or radio guarantee. Own-branch build and the fourteen-stage startup
fixture must precede any hardware claim; physical execution remains unobserved.

Own native continuation `e4cb5ea8` and consolidated `dd91a0a1` build the
complete x86 image. After raw publication `f566311f`, all twelve startup
fixtures passed (0.00 s, zero failed/ignored, x86-64 WSL2) including failure
at every one of the fourteen stages. This verifies orchestration behavior,
not actual firmware capabilities or a physical network link.

The shared XTAL command helpers now guard the early board-read cleanup as well
as later native cleanup: wait for an existing command before writing another,
with finite 50 ms polling inside the caller's total budget. A stuck command
retains primary and cleanup failures; successful power-off is not claimed.
Regression execution and physical validation remain pending for this slice.
