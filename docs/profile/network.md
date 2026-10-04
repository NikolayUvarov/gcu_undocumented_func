# Network performance — `x86-64/QEMU-0`

The network stack benchmark of issue 106 and what it decided. Appendix B.2: performance features are taken on the evidence of a measurement on a profile, not by default.

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

## Results (4 MiB each way, 2026-10-04)

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
- **Segmentation offload (TSO/GSO, mergeable receive buffers)** would put fewer, larger frames through the same per-frame path. It needs frames larger than the 1514 bytes of `idl/net.wit`, and smoltcp does not build TCP segments larger than the MTU. It is not done. The per-frame call is what to make cheaper first, with frames batched in shared rings between stack and driver ([issue 107](../../issues/107-batched-frame-path.md)).

## Decision

- **Transmit checksum offload** is implemented: `net.wit` 1.1 adds `offloads` and `send-partial`, `socket.wit` 2.2 adds `offload`, and the shell has `ip offload on|off`. `netstack` writes the pseudo-header sum and the card completes the checksum. Without offload the stack computes TCP and UDP checksums itself. The setting is **off** by default, because no gain beyond the noise was measured. Both settings are tested: the host's TCP stack drops segments with a wrong checksum, so the upload phase checks them.
- **Trust:**
  - With transmit offload, the card computes the checksums of what is sent. It already sees and could change those bytes: every DMA-capable driver and its device is in the TCB for memory isolation (MC-1.5, [tcb.md](tcb.md)). Offload adds no new trust.
  - Receive checksums are always verified by the stack. The card is not trusted to vouch for the integrity of received data, and `VIRTIO_NET_F_GUEST_CSUM` is not negotiated.
- **Segmentation offloads** are not used.
