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

## Configuration-specific acceptance

1. On ASTRA, run the full project gate, including all x86 QEMU groups and the
   required one-CPU suites; record the exact commit and per-group outcomes.
   Run aarch64 groups separately through Effector. Never run these on
   `fast-test` or infer hardware compatibility from them.
2. On Ryzen, compare the same workload and boot medium before/after H01:
   boot/report duration, program-menu opening, repeated small-file saves,
   typing responsiveness, WM timing and idle load. Record the manifest,
   hardware report and boot log for the new image.
3. On the MacBook, record ordinary Wi-Fi association/DHCP separately from the
   explicitly enabled radio diagnostic. Measure continuous ping and a bounded
   TCP transfer during the original test load; record losses, longest pause,
   reconnection and driver-restart recovery.
4. On Ryzen, obtain the RTL8168 TxConfig/XID and RTL8852BE silicon cut before
   claiming revision-specific initialization. Firmware files stay outside git
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
firmware-fetch tests, ten Wi-Fi evidence-collector tests and eight RTL8168
DMA/identity tests. The first x86 build of the audit fixes
passed; final gate and subsequent cache-transition changes require their own
record. External ConfigAgent's complete Go test suite and scaffold lint pass
on its isolated ASTRA clone; that server change has not been deployed.

No new hardware run, working Realtek driver, MIND endpoint registration or
successful Effector command on MIND is claimed by this audit.
