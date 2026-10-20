# Ryzen performance, MacBook networking and Effector audit — 2026-10-15

Requested by the maintainer as an auditor-fixer assignment on branch `ASTRA`.
Reviewed source: `main` at `94ec83a0`; recorded hardware image:
`e6730d7a1fa54301326b004c308adf483607d33f` (`fast-test`). The latter was read as
evidence only; no build, test or gate was run on `fast-test`.

## Evidence and limits

Read `F:\README.TXT`, `F:\boot0001.log` through `boot0005.log`, the matching
`hwNNNN.txt`, `E:\MANIFEST`, and `E:\data\bug-notes\ryzen-report.txt` and
`second-file.txt`. Read-only copies and their source paths, timestamps, lengths
and SHA-256 hashes are in `/tmp/mind-hardware-evidence-20261015/manifest.json`
on the audit machine. The first boot is the MacBook; the later boots are the
Ryzen. Raw logs, user text, network names and hardware identifiers are not
published in this repository. The source volumes were not written.

The maintainer reports slow menu/program startup, roughly five-second small
file saves, typing lag, initially jerky input, and about 15–17% idle desktop
load. Logs can establish device identity, startup events and some timings;
they do not measure every reported latency or reproduce lost ping under load.
Host tests and QEMU results below must not be carried over to either physical
machine (MC-12.1, MC-12.2, MC-12.3, MC-12.9).

## Findings and execution

| Finding | Assessment | Work and remaining evidence |
| --- | --- | --- |
| H01: Ryzen kernel RAM mapped uncached | **Confirmed, P1, KRN.** The old identity map forces every page above 3 GiB uncached. Ryzen places about 76.4 MiB of loader data there, including the entire 64 MiB kernel arena. | [213-KRN-0112](../issues/213-KRN-0112-firmware-ram-cache-types.md): firmware RAM classification, 4 KiB splits at mixed boundaries, and serialized boot cache transition. Hardware timing comparison remains. |
| H02: Unbounded network service work | **Confirmed, P1, NET.** The second stack poll and radio calls from client handlers escape the timeout scope. Fresh per-call timeouts do not bound a turn. Continuously replenished ingress can prevent a return to clients. | [550-NET-0031](../issues/550-NET-0031-bounded-driver-calls-and-ingress.md): shared deadlines, finite ingress bursts and malformed-ring yielding. MacBook loss/recovery under load still needs a measured run. |
| H03: Neither Ryzen NIC has a driver | **Confirmed, P1, DRV.** PCI `10ec:b852` is RTL8852BE Wi-Fi; `10ec:8168` is RTL8168 Ethernet. USB `0bda:b85b` is the Bluetooth function, not a usable Wi-Fi transport. | Wi-Fi first: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md), [0033](../issues/213-DRV-0033-rtl8852be-firmware-and-probe.md). Ethernet in parallel: [0030](../issues-done/213-DRV-0030-rtl8168-hardware-identification.done), [0031](../issues/213-DRV-0031-rtl8168-wired-network.md). Parsing/probe foundations are not working network drivers. |
| H04: Only one USB controller serves devices | **Confirmed coverage gap, P2, KRN/DRV.** A second xHCI exists; the current boot starts one host. This is consistent with port-dependent access, but does not establish the wiring of each physical socket. | Existing [213-KRN-0099](../issues/213-KRN-0099-a-usb-host-for-every-xhci.md) and its drivers request retain this work. Do not attribute it to the RAM cache fix. |
| H05: Keyboard placement lacks a visible mode | **Confirmed, P2, APP.** Alt+M previously retained the ordinary focused frame. | [000-APP-0081](../issues/000-APP-0081-visible-window-placement-mode.md): accent frame/title, including fullscreen overlay without resizing on entry. |
| H06: Dzen hour-sector expectation | **Partly confirmed, P2, APP.** The existing bottom-right marker intentionally marks the fixed zero of a 100-second cycle; it is not an hour pointer. There was no separate current-hour marker. | [000-APP-0080](../issues/000-APP-0080-dzen-current-hour-sector.md): distinct colored hour-sector marker; at 08:20 it points bottom-left. The original reference remains. |
| H07: Native Effector integration is incomplete | **Confirmed, P1, EFF.** No persistent MIND agent exists. Local ConfigAgent defaults can select foreign platform configuration/paths for a MIND endpoint. Operator API credentials are not native AgentToken credentials. | Existing [750](../issues/750-effector-agent.md), [contract](../docs/effector/protocol.md), HTTPS fixture and external ConfigAgent task 81. Runtime session, gateway, grants and live acceptance remain open. |
| H08: CPU-budget statement contradicts the implementation | **Confirmed, P2, documentation.** `docs/profile/threat-model.md` still says there are no CPU budgets. `scheduler.rs` implements per-task budgets, while the other profile documents correctly retain their limits. | Reported to the kernel owner in [requests-KRN](../issues/requests-KRN.md#cpu-budget-threat-model-wording-for-000-krn-0105), for existing [000-KRN-0105](../issues-done/000-KRN-0105-root-documents-agree-with-the-code.done). This is a stale profile statement, not evidence of full CPU-DoS protection or an explanation of the Ryzen slowdown. |

### H01: performance cause and falsifiable follow-up

The relevant Ryzen loader-data intervals include
`0xC20AB000–0xC68F5FFF` and `0xC68FB000–0xC6D0AFFF`. The firmware MTRR for
`0xC0000000–0xD0000000` is write-back; the old page flags override that with
uncached access. The MacBook's loader-data map does not have this overlap.
This makes the cache mapping a strong explanation for broad Ryzen kernel and
IPC slowness, but no speedup factor has yet been measured on that machine.

The USB drive enumerates at SuperSpeed and the FAT volume mounts at about
3.5 seconds. The logs do not show repeated storage-command failures explaining
every small-file delay. Hardware/ACPI report completion takes tens of seconds
on Ryzen versus a few seconds on the MacBook; slow report writes alone do not
separate storage latency from uncached kernel work. First retest after H01;
retain storage/IPC timing investigation if file saves remain slow.

### H02: what the MacBook log does and does not show

The image contains the explicitly enabled `data/wifitest.txt` diagnostic.
Its radio test begins at about 2 seconds and leaves the station busy until
about 117 seconds. This is deliberate disruptive testing, not ordinary idle
connectivity. The later log records association around 447.6 seconds and DHCP
around 457.0 seconds. It ends around 721 seconds without evidence establishing
the reported later ping failure. The boundedness defects are established from
code independently of that symptom; their repair is not a claim of measured
hardware reliability.

The per-card deadline also does not establish Effector heartbeat latency when
several drivers fail together: three two-second card turns can exceed the
server's five-second offline threshold. This remaining whole-service timing
requirement and a failed-peer/healthy-link test are recorded in
[750-EFF-0006](../issues/750-EFF-0006-session-service.md).

### H03: actual Linux identity after the maintainer rebooted Ryzen

On 2026-10-15, the maintainer made the Ryzen available through Effector in
Ubuntu 24.04.1 LTS, kernel `7.0.0-38-generic`. Read-only Linux observations
removed the revision uncertainty:

- RTL8168: TxConfig `0x4f000f80`, XID `0x4c0`, RTL8168g/8111g. The bound
  `r8169` driver reports PHY `0x001cc800` and `rtl8168g-2` firmware. The cable
  had no carrier; no wired traffic result is claimed. Task 0030 is complete;
  [0031](../issues/213-DRV-0031-rtl8168-wired-network.md) retains initialization,
  firmware, service integration and sustained-traffic validation.
- RTL8852BE: the exact-device boot record reports digital cut `CV=1`, analog
  cut `ACV=1`, `RFE=1`. The updated collector returned `identity-collected`
  with no warnings on the real machine. The pinned personal firmware parser
  selects CE cut 1, version `0.29.29.15`; Ubuntu's loaded `0.29.29.5` is a
  different file/version. The MIND raw `SYS_CFG1` observation and radio
  operation remain untested.

Operation IDs, timestamps and source mappings are in
[RTL8168 notes](../hwdocs/rtl8168.md#ryzen-observation-2026-10-15) and
[RTL8852BE notes](../hwdocs/rtl8852be.md). No driver unbind, reset, network
configuration change, debugfs transaction or raw MMIO was needed. Private
register bytes and machine reports remain outside git. Linux connectivity
does not establish MIND driver support.

## Configuration-specific acceptance

1. On ASTRA, record the exact commit and per-group outcomes, including all
   x86 QEMU groups and the required one-CPU suites. The maintainer explicitly
   prioritized Intel/x86 on 2026-10-15 and postponed ARM to a separate stage;
   i9 and Ryzen now divide the x86 groups. Keep the deferred ARM work and
   full cross-architecture gate open. Never run these on `fast-test` or infer
   hardware compatibility from them.
2. On Ryzen, compare the same workload and boot medium before/after H01:
   boot/report duration, program-menu opening, repeated small-file saves,
   typing responsiveness, WM timing and idle load. Record the manifest,
   hardware report and boot log for the new image.
3. On the MacBook, record ordinary Wi-Fi association/DHCP separately from the
   explicitly enabled radio diagnostic. Measure continuous ping and a bounded
   TCP transfer during the original test load; record losses, longest pause,
   reconnection and driver-restart recovery.
4. The Linux identity observations above now establish the RTL8168 XID and
   RTL8852BE cut/RFE. Collect the separate MIND identity report on the next
   boot before claiming the new MIND probe was exercised. Firmware files stay outside git
   and disk images, in the project's separate store and then on the written
   boot volume. Test Wi-Fi and Ethernet independently, including recovery and
   load. The maintainer requested Wi-Fi first and parallel Ethernet work.
5. Effector acceptance requires a real MIND heartbeat and command/result
   exchange, distinct native credentials, pinned TLS, a persistent replay
   journal and policy-limited gateway. The existing Linux build hosts are
   usable now; their accessibility says nothing about a MIND endpoint.

## Validation record

Focused checks passed during implementation: five actual page-table cache
regressions, four production network-budget tests, sixteen clock and three
pixel-renderer tests, forty WM tests, seven RTL8168 Linux-probe tests and
twelve HTTPS protocol-fixture tests, twelve RTL8852BE format tests, five
firmware-fetch tests, fourteen Wi-Fi evidence-collector tests and eight RTL8168
DMA/identity tests. The first x86 build of the audit fixes
passed. The final code candidate `1aea9100`, including the serialized cache
transition, passed all 24 standard host/x86 groups in the separate
[configuration-specific record](2026-10-15-astra-validation.md). ARM remains
explicitly deferred; the full cross-architecture gate is incomplete. External ConfigAgent's complete Go test suite and scaffold lint pass
on its isolated ASTRA clone; the maintainer authorized local commit
`50b51630dd27b827b83f8c8ad3532f74293ae271`. It has not been deployed. Active
ConfigAgent tasks `81-mindcore-01`, `81-mindcore-02`,
`82-linux-operation-timeout-01` and `82-linux-operation-timeout-02` remain
in the canonical `/home/un/common_virtual/_rdsc/sst-test-deploy/issues/`
queue with the Effector owner. That project integrated the server source as
`be9d62c` and handed native-agent work to MIND through `requests-EFF.md`
in main `bd7b5493`; source integration is not deployment.

The first full gate at `9a47d0dd` found an environmental network-test defect:
the host silently filters fixed TCP port 1. [000-ASR-0010](../issues/000-ASR-0010-owned-tcp-refusal-fixture.md)
reserves a non-listening ephemeral socket and retains the exact `Refused`
assertion. Captured packets establish the missing reset on port 1 and the
immediate reset on the reserved port. The isolated four-CPU QEMU network
suite passes, including 30 MiB HTTP/TLS interruption/resume, policy, multiple
cards and driver/stack restart. The corrected final host/x86 stage passed;
the full gate remains pending because ARM was deferred;
these checks do not turn the failed earlier gate into a passing one.

The same earlier gate also failed its TSC elapsed-rate check. An instrumented
repeat established a host realtime offset change: 31.470069 seconds of
realtime versus 30.050605 seconds of host monotonic time, with 29.772393
seconds in the guest. [000-ASR-0011](../issues/000-ASR-0011-monotonic-tsc-reference.md)
changes only the test's elapsed-time reference to the host monotonic clock,
retaining its sampling, 30-second interval, calibration check and 3% bound.
The corrected focused run passed with a ratio of 1.003.

A subsequent full candidate exposed the Dzen test's obsolete grayscale-only
orbit palette. Captured pixels matched the new intentional yellow hour
marker. The revised test requires that marker in its expected region and
retains all old orbit checks; the complete focused Dzen suite passed. These
failed candidates remain distinct from the final gate in
[the validation record](2026-10-15-astra-validation.md).

No new MIND hardware run, working Realtek driver, MIND endpoint registration
or successful Effector command on MIND is claimed by this audit.

## H09 — Realtek boot integration needs staged PCI authority and stable ABI layout

**Priority:** P1 · **Task:** [213-KRN-0114](../issues/213-KRN-0114-realtek-driver-startup.md)

Independent integration review found that MSI setup enabled mastering before
ring initialization, fixed Command bits rejected devices without both decode
spaces, and post-bootstrap restart attempted to mint a new Platform capability.
It also found a larger boot-image array shifting the ABI-version field, which
made version rejection occur after invalid pointer use. The ASTRA integration
now preserves the ABI 10 prefix, checks the version first, appends image slots,
uses BAR-authorized MEM-only restart, and leaves Command untouched during MSI.
Init stops devices before restart-budget quarantine and clears DMA only after
the old process has ended and device stop/mapping succeed. Focused PCI and ABI
fixtures passed (8 PCI-control and 6 ABI cases); the later integrated runs
and their limits are recorded in [the network validation](2026-10-16-network-validation.md).
No physical Realtek guarantee follows from these software changes.

## H10 — Safe-start devices have no retained launch plan for later operator start

**Priority:** P2 · **Request:** [kernel coordination](../issues/requests-KRN.md)

`init` skips safe-start services before hardware discovery or capability plans
are created. After dropping Platform, `svc start bcm_wifi` cannot rediscover its
device; the same limitation applies to the added RTL8852BE board-stage service.
This existing behavior contradicts the old comment that each omitted service
can be started later. The comment is corrected; a kernel-owner request asks for
preparing narrow restart authority without starting the risky device. A normal
subsequent boot remains distinct from an operator start within safe mode.

## H11 — Accepted WLAN requests wait behind background receives

**Priority:** P1 · **Task:** [550-NET-0033](../issues/550-NET-0033-wlan-client-and-bulk-fairness.md)

The station previously accepted a client, then received up to 64 radio
frames under a two-second deadline before dispatching that client. The
stack's fair-turn deadline can expire after 250 ms. Client-first dispatch
and a cooperative eight-frame/100 ms receive batch now remove that queued
work from the front of an already accepted request. All 26 network-budget
and 12 unchanged station protocol host cases passed after publication.
One slow synchronous call can still exceed the slice; asynchronous radio
progress remains separate. This finding is not yet a measured explanation
of the original MacBook's packet loss, nor evidence of working Ryzen Wi-Fi.
