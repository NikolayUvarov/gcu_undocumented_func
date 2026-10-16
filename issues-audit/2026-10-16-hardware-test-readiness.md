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

The combined source candidate `c4933c2bca66f0cd0d81bb9a5ca33f8c587a5052`
built on ASTRA before publication in fast-test
`3c6eaa5648c240670dc29fd59d6b17954a01da2f`. It incorporates the sustained-H2C
fixture corrections and the composed connection-to-service test. All nine
selected integration groups passed; the [exact report](../docs/validation/2026-10-16-astra-x86-targeted.md)
records host/TLC and four QEMU groups, configurations, operation IDs and retained
failures. This targeted result is separate from the earlier [x86 matrix](2026-10-16-network-validation.md),
which retains a RAM-above-4-GiB download failure and is not a green gate.
The superseded `d53d9f45` host run retains the exhausted synthetic-traffic
assertion; it is not counted as a pass. Neither a model result nor a QEMU run
establishes physical traffic on either target machine.

## Current boundaries

| Path | Implemented | Still required before claiming working networking |
|---|---|---|
| Intel MacBook / BCM4331 | Owner's driver and startup, stack deadlines and station dispatch fairness are integrated. | Owner correction of [H12](2026-10-16-bcm-tx-wait-audit.md); boot, association/DHCP, ping under the original workload, and recovery on this MacBook. H12 is not established as the observed loss's cause. |
| Ryzen / RTL8168G | Exact XID/PHY guards, bounded controller waits, retained DMA, staged bus mastering, native card service and stack endpoint. [H15](2026-10-16-rtl8168-deadline-audit.md) has a tested correction. | Cable/link negotiation, DHCP, bidirectional ping/TCP/TLS, sustained load and unplug/replug/restart on this device. Linux identification and modeled rings do not establish MIND traffic. |
| Ryzen / RTL8852BE | Native board/firmware/MAC/RX/initial-PHY diagnostic; private channel/calibration/data coordinator, generated-wire wlanhw service, bounded runtime coexistence and retained TX/RX queues. | Device-bound platform policy, consumption of the confirmed UZ selection and admitted channel; native endpoint and service activation; then association, IP traffic, load and recovery. The executable still stops after its diagnostic. |
| MIND / Effector | Server-boundary work and contract foundations. | Working MIND networking first, then native agent credentials, pinned TLS, heartbeat, bounded command/configuration and reconnect acceptance. Linux Effector availability is not MIND-agent readiness. |

The Ethernet controller uses the reviewed BSD PHY baseline; Linux PHY patch
and OCP/ERI/EPHY tuning are not implemented. Its cold-start adequacy is unknown,
so a failed physical link may require additional controller/PHY implementation,
not merely more measurements. See [the baseline limits](../hwdocs/rtl8168.md).

The Wi-Fi board stage rejects inherited active MAC power state; memory decoding
with bus mastering disabled does not establish an inactive MAC. Warm-state reset
is not implemented. Record cold/warm boot and the previous OS/driver separately;
a warm-state refusal must not be misreported as a calibrated radio failure.

The Wi-Fi policy and endpoint work is requested from its owning track in
[requests-KRN](../issues/requests-KRN.md#device-bound-platform-power-policy-for-the-ryzen-wlan-driver)
and [the native endpoint request](../issues/requests-KRN.md#native-rtl8852be-radio-endpoint-after-datapath-admission).
The maintainer confirmed Uzbekistan (`UZ`) for both machines on 2026-10-16.
That explicit choice supersedes the collected Ubuntu DE setting as deployment
input. The maintainer also requested switching among every country in the
verified regulatory database; [NET](../issues/requests-NET.md#wifi-country-selection-from-the-complete-verified-regulatory-catalog),
[APP](../issues/requests-APP.md#wifi-country-and-channel-selector) and
[BCM driver](../issues/requests-DRV.md#bcm4331-applies-the-selected-country-and-channel-policy)
requests carry the configuration, UI and original MacBook portions. The new
RTL [catalog implementation](../hwdocs/rtl8852be-regulatory.md), source
`1e2cf1dd`, is separate from tested candidate c4933c2b. It retains every one of
the 181 country/territory codes and separate world `00`, all 1,016 source rule
references, source hashes and licence notice. ASTRA merge `4ece2057` built,
then fast-test `94ab2a0b` was pushed before the focused 9 Rust and 11 Python
fixtures and exact regeneration passed on the source branch. This catalog
provides selection and restrictions; persistent configuration, UI and native
radio application remain pending owner work. A selected country does not
replace platform limits: missing policy data is not a checked absence.
The private service supports one explicitly admitted 2.4 GHz 20 MHz channel;
it must reject an unsupported channel rather than silently retune.

The [association boundary](../hwdocs/rtl8852be-data-mode.md#association-boundary)
also remains unmeasured: software WPA2 handles authentication/CCMP, while the
new driver's initial station is NO_LINK/AID0. Its raw-frame tests do not
establish association, and they do not by themselves justify a new WLAN ABI.

## Physical acceptance record

For each machine/path, preserve the exact image manifest, boot/driver logs,
firmware versions, cold/warm boot history and test configuration. Record failures as well as passes.

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
