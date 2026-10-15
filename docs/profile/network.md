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
not vendor-signature verification. Build/host evidence for this library slice
is recorded separately by its owner; actual Ryzen firmware/radio operation remains pending;
the original board executable at that stage performed no download or host DMA.

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
The integration branch builds `rtw89_wifi` and `init` for x86-64; its ten
startup/activation host cases are compiled and await execution after publication.
