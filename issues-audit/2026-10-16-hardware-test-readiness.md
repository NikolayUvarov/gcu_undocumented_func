# Intel MacBook and Ryzen network test readiness — 2026-10-16

**Owner:** ASTRA auditor-fixer. **Roadmap:** H / 213, D / 550, 750.
**Constitution:** MC-6.1, MC-12.1, MC-12.3, MC-12.9.
**Status:** integration in progress; physical network readiness is not established.

## Scope and publication

The original Intel MacBook and the identified Ryzen are the hardware targets.
ARM remains explicitly deferred by the maintainer. ASTRA changes are built on
ASTRA, merged and pushed into fast-test before their own-branch tests. No agent
builds, tests or gates fast-test. The maintainer's build-and-write script remains
the hardware-image path; firmware is fetched privately and copied only after
writing the image, with hash checks. No firmware payload enters Git or the image.

The combined source candidate `d53d9f45a17e55f9883681051d5143e1aacd71d4`
built on ASTRA before publication in fast-test
`4a4fa150bc430660efe696d302041a9219c36711`. Its integration tests are tracked
separately from the earlier [x86 matrix](2026-10-16-network-validation.md),
which retains a RAM-above-4-GiB download failure and is not a green gate.
A subsequently corrected fixture must receive its own publication and record;
neither a superseded test candidate nor a model result is hardware evidence.

## Current boundaries

| Path | Implemented | Still required before claiming working networking |
|---|---|---|
| Intel MacBook / BCM4331 | Owner's driver and startup, stack deadlines and station dispatch fairness are integrated. | Owner correction of [H12](2026-10-16-bcm-tx-wait-audit.md); boot, association/DHCP, ping under the original workload, and recovery on this MacBook. H12 is not established as the observed loss's cause. |
| Ryzen / RTL8168G | Exact XID/PHY guards, bounded controller waits, retained DMA, staged bus mastering, native card service and stack endpoint. [H15](2026-10-16-rtl8168-deadline-audit.md) has a tested correction. | Cable/link negotiation, DHCP, bidirectional ping/TCP/TLS, sustained load and unplug/replug/restart on this device. Linux identification and modeled rings do not establish MIND traffic. |
| Ryzen / RTL8852BE | Native board/firmware/MAC/RX/initial-PHY diagnostic; private channel/calibration/data coordinator, generated-wire wlanhw service, bounded runtime coexistence and retained TX/RX queues. | Device-bound platform policy and explicit operator country/channel; native endpoint and service activation; then association, IP traffic, load and recovery. The executable still stops after its diagnostic. |
| MIND / Effector | Server-boundary work and contract foundations. | Working MIND networking first, then native agent credentials, pinned TLS, heartbeat, bounded command/configuration and reconnect acceptance. Linux Effector availability is not MIND-agent readiness. |

The Wi-Fi policy and endpoint work is requested from its owning track in
[requests-KRN](../issues/requests-KRN.md#device-bound-platform-power-policy-for-the-ryzen-wlan-driver)
and [the native endpoint request](../issues/requests-KRN.md#native-rtl8852be-radio-endpoint-after-datapath-admission).
The Ubuntu regulatory setting is not an operator declaration of physical
country. Missing policy data is not a checked absence of platform limits.
The private service supports one explicitly admitted 2.4 GHz 20 MHz channel;
it must reject an unsupported channel rather than silently retune.

The [association boundary](../hwdocs/rtl8852be-data-mode.md#association-boundary)
also remains unmeasured: software WPA2 handles authentication/CCMP, while the
new driver's initial station is NO_LINK/AID0. Its raw-frame tests do not
establish association, and they do not by themselves justify a new WLAN ABI.

## Physical acceptance record

For each machine/path, preserve the exact image manifest, boot/driver logs,
firmware versions and test configuration. Record failures as well as passes.

1. Boot and record driver identity, startup/cleanup outcomes, service state,
   address, route and link. A successful Wi-Fi diagnostic is a diagnostic result.
2. Test Wi-Fi and Ethernet independently. On a usable path record DHCP,
   bidirectional ping, a bounded TCP transfer and authenticated HTTPS.
3. Keep ping running alongside the original slow workload for at least
   30 minutes; record loss, longest interruption, latency, throughput and NIC
   counters. Repeat the Ryzen boot/menu/file-save/input/idle-load measurements
   from the [original audit](2026-10-15-hardware-audit.md#configuration-specific-acceptance).
4. Record link loss/recovery and driver restart. A timeout must leave the
   system responsive and retain uncertain DMA ownership; reconnection must
   establish a new usable path.
5. Test native Effector only once that same MIND network path works. Record a
   real heartbeat and command/result exchange, not the Linux build-host session.

Keep [DRV-0031](../issues/213-DRV-0031-rtl8168-wired-network.md),
[DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md) and
[KRN-0114](../issues/213-KRN-0114-realtek-driver-startup.md) open until their
own acceptance criteria are met. Current experimental fast-test publication
is not an assertion that both machines are ready for network acceptance.
