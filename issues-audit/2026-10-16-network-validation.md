# ASTRA network integration validation — 2026-10-16

**Status: partial software validation; neither target's physical networking
has been accepted.** This record serves MC-12.1, MC-12.3 and MC-12.9. The
maintainer prioritizes the original Intel MacBook and Ryzen; ARM is deferred.
The [earlier 24-group result](2026-10-15-astra-validation.md) belongs only to
`1aea9100` and is not a gate for the following driver/ABI changes.

## Candidate a99cc44a: completed subset, then superseded

Candidate `a99cc44a650b2d4cf287116dfa7adb520277ca27` includes ABI 11,
Realtek startup, the initial RTL8168 service and RTL8852BE board reads.
Baseline main is `bd7b5493d22f5d818c5c07b07ba4873dbaba4b60`.
It was published in fast-test `d8494d6e8dc5ea323d3c7aa2ae4014b4cd567a3b`
before tests on ASTRA/its dedicated test worktrees. No fast-test build or
test was performed. The host configurations and pinned nightly are those
in the earlier validation record; the following results stand separately.

| Group | Host | Result | Time | Operation |
|---|---|---|---|---|
| build (x86) | Ryzen Ubuntu | PASS | 200 s | `20261015210006.160525277_000124` |
| network driver without legacy | Ryzen Ubuntu | PASS | 5 s | `20261015210006.160525277_000124` |
| host tests | Ryzen Ubuntu | PASS | 156 s | `20261015210006.160525277_000124` |
| models (TLC) | Ryzen Ubuntu | PASS | 142 s | `20261015210006.160525277_000124` |
| build (x86 test programs) | Ryzen Ubuntu | PASS | 43 s | `20261015210006.160525277_000124` |
| x86: TSC measured again | Ryzen Ubuntu | PASS | 47 s | `20261015210933.230770724_000130` |
| x86: boot, display, network, TLS, shell, memory, clock | Ryzen Ubuntu | PASS | 827 s | `20261015211034.394143688_000131` |
| build (x86) | Intel i9 WSL2 | PASS | 221 s | `20261015205951.445363469_000123` |
| build (x86 test programs) | Intel i9 WSL2 | PASS | 50 s | `20261015205951.445363469_000123` |
| x86: keys, shell, tools | Intel i9 WSL2 | PASS | 1435 s | `20261015210623.326988165_000128` |
| x86: reproducible build | UN-LEGION9 WSL2 | PASS | 181 s | local `ci_local.sh`, 109 identical files |

The Intel group's first submission, `20261015210503.403869392_000126`,
failed in rustup's update preflight with a proxy error before QEMU started.
The unchanged retry passed; the first operation is not counted as a test
pass. The Intel build manifest remained unchanged before/after the group:
125 files, SHA-256
`5351823b9a759e9a8df6102c4add2c73df73dfe71aaea79cf98baf2327dc9e08`.
Focused production PCI-control (8) and boot-prefix/early-framebuffer (6)
fixtures also passed on UN-LEGION9 after publication.

No further old-candidate groups were submitted after the terminal groups
above. New WLAN fairness and the original drivers owner's BCM4331 hardware
dispatcher were integrated next. In particular, the tested a99cc44a ASTRA
tree did not yet contain that hardware dispatcher; QEMU wlansim results
cannot establish the original MacBook's readiness. The later standard
x86 matrix has 25 groups, including the kernel owner's watchdog group.
This subset is neither that full matrix nor a cross-architecture gate.

## Candidate 0c4205e7: newer integration and a retained failure

`0c4205e77322c3f751d26990e139e677d8bb30e6` adds the original DRV branch
at `5623c9ba`, NET0033 client-first WLAN dispatch, bounded receive batches,
and later Wi-Fi preparation helpers. It builds on ASTRA and was published
in fast-test `f2d4164a01647247cb22ac00538ef6b630520e8d` before testing.

On UN-LEGION9, `ci_local.sh --only x86` with the fixture-build and primary
QEMU groups passed the fixture build (31 s), then **failed** the primary
group after 557 s. Boot, display and the complete network suite emitted
their passing summaries; the TLS suite then timed out on its hard-coded
`https 10.0.2.2:1` refusal fixture. The remaining group suites were not run.

The same host's native port 1 connect timed out after 1.000450 s; an owned,
non-listening ephemeral port returned ECONNREFUSED after 0.000582 s.
[000-ASR-0010](../issues/000-ASR-0010-owned-tcp-refusal-fixture.md) extends
the already established network fixture fix to TLS, retaining the exact
`HTTPS: Refused` assertion and default timeout. Implementation `5370bd25`
passed Python compilation and the own-branch full build, and was published
in fast-test `ffb7f540` before its separate focused TLS run. That complete
four-CPU TLS suite passed on UN-LEGION9 with exit 0, including certificate
validation/ciphers, refused TCP, client certificate authority, persistent
device keys, no-entropy refusal and TPM sealing. Its log is
`/tmp/mind-tls5370-suite.log`; command: `TMPDIR=/tmp/mind-tls5370 python3
tests/qemu_smoke.py --qemu qemu-system-x86_64 --firmware
/usr/share/ovmf/OVMF.fd --cpus 4 --suites tls`. This correction does not
retrospectively turn the failed 0c4205e7 group into a pass.

## Candidate ac1854f2: completed x86 matrix, one retained failure

Candidate `ac1854f202ac8ceebb2167c8b3e0da4a539fdd33` with baseline main
`bd7b5493d22f5d818c5c07b07ba4873dbaba4b60` was published in fast-test
`2f0e20d8faa1c188bdb8428daf76b8caf6023a45` before these own-branch runs.
It includes the TLS refusal correction and the original BCM dispatcher.
All 25 scoped host/x86 groups reached a terminal result: **24 PASS, one FAIL**.
ARM remains deferred; this is not a green gate and authorizes no code push
to main. Later PHY/MAC/RX/BTC slices are not covered by this frozen matrix.

| Group | Host | Result | Time | Operation |
|---|---|---|---|---|
| build (x86) | Ryzen Ubuntu | PASS | 200 s | `20261015214652.934670562_000138` |
| network driver without legacy | Ryzen Ubuntu | PASS | 5 s | `20261015214652.934670562_000138` |
| host tests | Ryzen Ubuntu | PASS | 173 s | `20261015214652.934670562_000138` |
| models (TLC) | Ryzen Ubuntu | PASS | 142 s | `20261015214652.934670562_000138` |
| build (x86 test programs) | Ryzen Ubuntu | PASS | 44 s | `20261015214652.934670562_000138` |
| x86: TSC measured again | Ryzen Ubuntu | PASS | 47 s | `20261015215634.814129423_000143` |
| x86: boot, display, network, TLS, shell, memory, clock | Ryzen Ubuntu | PASS | 839 s | `20261015215733.582403682_000144` |
| x86: services, storage, audio | Ryzen Ubuntu | PASS | 182 s | `20261015221136.838502617_000149` |
| x86: scheduling, isolation, heap | Ryzen Ubuntu | PASS | 97 s | `20261015221448.229680138_000150` |
| x86: files and block writes | Ryzen Ubuntu | PASS | 548 s | `20261015221627.872067071_000151` |
| x86: one CPU | Ryzen Ubuntu | PASS | 114 s | `20261015222543.169891176_000155` |
| x86: AVX state | Ryzen Ubuntu | PASS | 41 s | `20261015222756.085713267_000156` |
| x86: AVX state, one CPU | Ryzen Ubuntu | PASS | 33 s | `20261015222756.085713267_000156` |
| x86: no PIT | Ryzen Ubuntu | PASS | 121 s | `20261015222921.772814608_000157` |
| x86: x2APIC | Ryzen Ubuntu | PASS | 169 s | `20261015223142.294857768_000158` |
| x86: padded vector area | Ryzen Ubuntu | PASS | 73 s | `20261015223441.419534525_000160` |
| x86: protection probes | Ryzen Ubuntu | PASS | 32 s | `20261015223600.363663523_000162` |
| x86: the watchdog | Ryzen Ubuntu | PASS | 123 s | `20261015223635.383529657_000163` |
| x86: USB image | Ryzen Ubuntu | PASS | 81 s | `20261015223844.128174470_000165` |
| x86: Secure Boot with our keys | Ryzen Ubuntu | PASS | 20 s | `20261015224018.231645011_000166` |
| x86: keys, shell, tools | Intel i9 WSL2 | PASS | 1430 s | `20261015215323.563088752_000141` |
| x86: RAM above 4 GiB | Intel i9 WSL2 | FAIL | 715 s | `20261015221739.924407850_000152` |
| x86: NVMe boot disk | Intel i9 WSL2 | PASS | 65 s | `20261015223554.903169564_000161` |
| x86: 16 CPUs | Intel i9 WSL2 | PASS | 154 s | `20261015223731.347774261_000164` |
| x86: reproducible build | UN-LEGION9 WSL2 | PASS | 175 s | `local, 109 identical files` |

Ryzen ran native Ubuntu 24.04, Linux 7.0.0-38-generic, QEMU 8.2.2
(Ubuntu) under TCG. Intel ran Ubuntu 24.04 under WSL2 with the same QEMU
package and TCG. Its independent build prerequisites also passed (223 s
x86 build, 48 s fixtures, operation `20261015214832.111053736_000139`).
The Intel image manifest remained 125 files, SHA-256
`08ee8e3953865138f1db3d7463be3abaa5cffde6c4f29211152fcbb997e45677`.
Local reproducibility compared 109 identical files across two x86 builds.
No tests or builds ran on fast-test, and no machine was rebooted.

The 6 GiB group passed its memory prechecks and first resumed 30 MiB transfer
and hash, then stalled in the already-complete second download after HTTP 416.
The existing 300-second wait expired without a digest or prompt.
[Audit H13](2026-10-16-download-recheck-stall.md) preserves the exact failure
and the NET request to distinguish flush, VFS read/hash and scheduling.
Independent NVMe and 16-CPU groups continued; their passes do not erase the
RAM-group failure. Neither QEMU nor host models establish physical NIC traffic.

Final structured records: `ryzen-ac18-groups.json`,
`ryzen-ac18-final-gate-results.json`, `astra-i9x3-result.json` and
`local-repro-ac18-result.json` in the private evidence directory below.

## Hardware readiness boundary

| Target/path | Implemented work | Acceptance still required |
|---|---|---|
| Original Intel MacBook / BCM4331 | Owner's actual driver/startup merged; stack deadlines and WLAN dispatch fairness; firmware copied after writing the disk | Same hardware boots, associates, gets an IP address; ping during the reported workload; loss/recovery and service restart |
| Ryzen / RTL8168G Ethernet | Exact XID/PHY guards, ring initialization, delayed bus mastering, retained DMA and stack endpoint | Cable/link negotiation, DHCP, ping/TCP/TLS under load and restart on MIND |
| Ryzen / RTL8852BE Wi-Fi | Exact cut/RFE board reads; pinned-file verification; reviewed firmware transport, PCIe and PHY library stages; bounded native firmware diagnostic in `5f1d940d` | Diagnostic hardware acceptance, complete MAC/RX/C2H, calibration/channel/power constraints, WLAN service, association and IP traffic |
| Both / Effector | Server boundary and session-library work | Usable network first; native agent, reconnect and live command/configuration acceptance afterward |

Library fixtures do not establish any physical row above. Firmware stays
outside Git and disk images; the standard post-write path copies the pinned
files from the maintainer's private store. Each subsequent implementation
must build on its own branch, merge/push to fast-test, and only then run its
tests on that branch. The maintainer alone builds the fast-test image.

## Evidence locations

Local run records are under `/tmp/mind-astra-private-20261015/`:
`ryzen-a99-final-gate-results.json`, `astra-i9x2-result.json`,
`local-repro-a99-result.json` and `boot-abi-a99cc44a-result.json`.
The failed integrated log is
`/tmp/mind-net0c42/logs/02-x86__boot__display__network__TLS__shell__memory__clock.log`.
Effector operation IDs above identify the remote records. These private
records contain no published credentials; local temporary paths are not
a substitute for recording final outcomes in the repository.
