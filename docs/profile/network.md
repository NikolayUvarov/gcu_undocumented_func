# Network performance — `x86-64/QEMU-0`

The network stack benchmark of issue 106 and what it decided. Appendix B.2: performance features are taken on the evidence of a measurement on a profile, not by default.

## Bounded service work (550-NET-0031, 2026-10-15)

On ASTRA, each card turn has a two-second deadline shared by all of its driver calls. Calls after expiry are skipped; the five-second cooldown applies only to an attempted IPC that times out, not to a healthy card whose work budget expires. The stack processes at most 64 incoming packets before one egress pass and the next card. WLAN shares a turn deadline across its radio and key-store calls, including client request handlers. These bounds limit service work; they are not a real-time scheduling guarantee.

Four host tests exercise the production deadline and smoltcp polling helper with slow or silent peers and continuously arriving packets (`tests/network_budget/Cargo.toml`); the two affected services build for x86-64. QEMU integration and hardware latency/loss measurements remain pending. The supplied MacBook log does not establish the cause of the reported ping loss under load, and these changes do not add a driver for the Ryzen's Realtek NICs.

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

The separate channel-12 command queue now has a bounded descriptor/payload
ownership implementation but is not called by the native service. Its TCB
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
invocation correction and precise scope. It is
not called by the native board service and supplies no host DMA or radio
startup; actual hardware operation and the subsequent PCIe/firmware stages
remain unvalidated.

The later PCIe preparation implements the pinned RTL8852B MDIO/DBI path,
temporary L1 restoration, channel stops/index/mode setup and BDRAM reset
after command-ring installation. It checks the MAC download profile and
preserves PCI MEM-only/HCI-off state. Primary and independent restoration/
stop failures remain explicit, and no DMA map is freed. Build/host evidence
for this additional slice is pending; it is not invoked by the native service
and provides no tested hardware DMA, firmware download or radio operation.
