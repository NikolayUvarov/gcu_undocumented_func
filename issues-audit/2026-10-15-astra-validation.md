# ASTRA hardware audit validation — 2026-10-15

**Status: all 24 host/x86 acceptance groups passed; ARM deferred by the maintainer. This is not a full green gate.** This record serves MC-12.1, MC-12.3
and MC-12.9 and accompanies the [hardware audit](2026-10-15-hardware-audit.md).

## Candidate and configurations

- Candidate: `1aea910037edcccce4597998b5190b911d659330` on `ASTRA`.
- Main merged for testing: `94ec83a00f7f828772af88d43af9f4a0bd5fc394`.
- Host/x86: `un-rizen_2a8b108d5593` through Effector, Ubuntu 24.04.1 LTS,
  native Linux `7.0.0-38-generic`, QEMU 8.2.2 (Ubuntu package
  `1:8.2.2+ds-0ubuntu1.18`), TCG; ordinary unprivileged `un` operations.
  The installed OpenSSH test server is `9.6p1 Ubuntu-3ubuntu13.19` at
  `/usr/sbin/sshd`. No package or service was installed for this run.
- Intel host: `i9_4e6882212251`, Intel i9-9900K with 16 logical CPUs,
  through Effector, Ubuntu 24.04.3 LTS under WSL2,
  Linux `6.6.87.2-microsoft-standard-WSL2`, QEMU 8.2.2 (Ubuntu package
  `1:8.2.2+ds-0ubuntu1.18`), OVMF `2024.02-2ubuntu0.10`, TCG;
  separate configuration from native Ryzen. After the maintainer prioritized
  Intel/x86 on 2026-10-15, this host runs the x86 keys/shell/tools, RAM above
  4 GiB, NVMe and 16-CPU groups in parallel with Ryzen, after its own x86
  build and fixture prerequisites. Its earlier aarch64 work is deferred.
  The OVMF file's SHA-256 is
  `7104ea759145e347d036a48045c38a7c24fb8885b314a9366f61e5b682429186`.
- PCU: `PCU_585240b00e8a`, Intel Celeron N5095A with four cores, Ubuntu
  22.04.5 LTS, native Linux `6.8.0-52-generic`, QEMU 8.2.2, OVMF
  `2022.02-3ubuntu0.22.04.7`. Its fresh build was too slow to shorten the
  critical path and was cancelled; no files/block test was submitted and
  no result from this host is credited. Its ASR operation used the required
  sequential queue. Cancellation cleanup verified zero remaining owned
  processes before the files/block group returned to Ryzen. OVMF SHA-256:
  `131d507f06fd7f09e5ac718266cfa520484e7e68ab3bc219b59f416ab06af374`.
- Local build host: `UN-LEGION9`, Intel Core Ultra 9 275HX, Ubuntu 24.04.4 LTS
  under WSL2 (`6.6.87.2-microsoft-standard-WSL2`). It runs only the standard
  x86 reproducible-build group through `ci_local.sh --ref ASTRA`, using two
  fresh checkouts of the exact candidate. All 106 output files matched byte
  for byte; this group passed in 183 seconds. It is a different machine from i9.
- Rust toolchain: the repository's `nightly-2026-10-02`.
- First chunks use `scripts/ci_local.sh --ref ASTRA --keep`; subsequent
  chunks use its retained merged worktree with `--tree` and selected groups.
  Chunks retain the exact commit and use separate log directories. All
  24 x86/host groups below passed for this stage. Operations stay below one
  hour and run sequentially within each machine's ASR work, with the three x86
  hosts working in parallel. No tests or builds run on `fast-test`. The normal
  cross-architecture project gate remains incomplete; `main` is not updated
  on the basis of this x86-only stage.
- QEMU uses four CPUs except each explicitly named alternative. The one-CPU
  group includes SMP, isolation, heap and services. These are emulator results,
  not physical Ryzen/MacBook performance or radio acceptance.

The tested code was already published in `fast-test` merge `821e9725` before
these final runs. The tests used ASTRA and retained test worktrees only.
While the runs were finishing, `main` advanced to `bd7b5493` with three
Markdown handoff files; the test worktrees stayed at the named `1aea9100`
candidate and `94ec83a0` baseline. Later native-agent/driver changes need
their own build and tests; these results do not cover them.

## x86 acceptance groups

<!-- ASTRA_X86_RESULTS_START -->

| Group | Host | Result | Duration | Run / operation |
|---|---|---|---|---|
| build (x86) | Ryzen | PASS | 196 s | `20261015192334.733149216_000066` |
| network driver without legacy | Ryzen | PASS | 5 s | `20261015192334.733149216_000066` |
| host tests | Ryzen | PASS | 146 s | `20261015192334.733149216_000066` |
| models (TLC) | Ryzen | PASS | 147 s | `20261015192334.733149216_000066` |
| build (x86 test programs) | Ryzen | PASS | 39 s | `20261015192334.733149216_000066` |
| x86: boot, display, network, TLS, shell, memory, clock | Ryzen | PASS | 820 s | `20261015193335.731294428_000073` |
| x86: services, storage, audio | Ryzen | PASS | 181 s | `20261015194730.003565054_000087` |
| x86: scheduling, isolation, heap | Ryzen | PASS | 96 s | `20261015195046.630378512_000091` |
| x86: keys, shell, tools | i9 | PASS | 1442 s | `20261015194214.897107783_000082` |
| x86: files and block writes | Ryzen | PASS | 547 s | `20261015200851.117364825_000114` |
| x86: NVMe boot disk | i9 | PASS | 65 s | `20261015201909.109850083_000121` |
| x86: 16 CPUs | i9 | PASS | 153 s | `20261015202047.653765303_000122` |
| x86: RAM above 4 GiB | i9 | PASS | 731 s | `20261015200652.646403488_000112` |
| x86: one CPU | Ryzen | PASS | 114 s | `20261015195227.365644006_000092` |
| x86: AVX state | Ryzen | PASS | 40 s | `20261015195426.678416769_000094` |
| x86: AVX state, one CPU | Ryzen | PASS | 33 s | `20261015195426.678416769_000094` |
| x86: no PIT | Ryzen | PASS | 119 s | `20261015195555.901514872_000096` |
| x86: x2APIC | Ryzen | PASS | 168 s | `20261015195949.242223344_000100` |
| x86: padded vector area | Ryzen | PASS | 73 s | `20261015200256.064135485_000106` |
| x86: protection probes | Ryzen | PASS | 31 s | `20261015200550.926982463_000110` |
| x86: TSC measured again | Ryzen | PASS | 47 s | `20261015193238.156862195_000072` |
| x86: USB image | Ryzen | PASS | 81 s | `20261015200641.540727783_000111` |
| x86: Secure Boot with our keys | Ryzen | PASS | 19 s | `20261015200817.116188328_000113` |
| x86: reproducible build | UN-LEGION9 | PASS | 183 s | `local ci_local.sh run` |

The other hosts also run their own x86 build and fixture prerequisites;
results below do not replace the Ryzen host-test configuration.

| Host | Prerequisite | Result | Duration | Effector operation |
|---|---|---|---|---|
| i9 | build (x86) | PASS | 215 s | `20261015193725.758370524_000078` |
| i9 | build (x86 test programs) | PASS | 43 s | `20261015193725.758370524_000078` |
| PCU | build (x86) | cancelled | — | `20261015194811.792491872_000088` |
| PCU | build (x86 test programs) | not run | — | `20261015194811.792491872_000088` |

<!-- ASTRA_X86_RESULTS_END -->

## Deferred ARM validation

The maintainer explicitly postponed ARM on 2026-10-15 to prioritize Intel/x86.
Operation `20261015192328.209264971_000065` on candidate `1aea9100` was cancelled
at 19:36:02 UTC. Its logs explicitly reported build and boot/fault-containment
PASS; the programs/shell/four-CPU group was interrupted and the later groups
were not started. No final duration table or full ARM pass is claimed. A
read-only post-cancel check found zero remaining processes of this run before
the i9 was reassigned to x86. The eight standard ARM groups remain deferred
for a separate run; older candidates' passing groups do not fill this gap.

## Earlier candidate and focused follow-up

The first full host/x86 run used `9a47d0dd` and finished with 21 passing
and three failing groups. Ordinary x86 networking and the network portion
of the 6 GiB group failed because the host dropped connections to fixed TCP
port 1 rather than returning a reset. Task 000-ASR-0010 reserves a
non-listening loopback socket and still requires `FETCH: Refused`. Packet
capture and a complete isolated four-CPU network suite established the
fixture correction; no TCP implementation or expected result was weakened.

The third failure was the TSC elapsed-rate assertion (ratio 0.95455). An
instrumented repetition recorded host realtime advancing 31.470069 seconds
while host monotonic time advanced 30.050605 seconds and the guest advanced
29.772393 seconds. Task 000-ASR-0011 uses the monotonic host reference with
the original interval, calibration assertion and three-percent bound. The
initial failure is not erased or treated as a passing result.

Candidate `229d90e8` passed all four host groups, the x86 fixtures, services /
storage / audio and scheduling / isolation / heap. Its main QEMU group failed
at the Dzen screenshot after boot, display, network, TLS, normal and memory
checks had passed. The new intentional hour-sector marker was rejected by an
obsolete grayscale-only assertion. A focused capture found exactly the eight
expected yellow top-right pixels; the corrected test requires that marker,
its erasure and all old orbit checks. The complete focused Dzen suite passed
in operation `20261015191446.779635445_000053` (36.7 s). Candidate `b6b08c86`
contains that correction. Its gate was cancelled before completion to include
the separately established monotonic-clock fixture correction.

Earlier passing groups are not substituted for the final candidate's rows
above. Both superseded ARM runs were explicitly cancelled, and their process
trees were verified absent before replacement operations were started.

Before the final gate, the Wi-Fi collector passed fourteen host tests and ran
on the actual Ryzen with `identity-collected`, exit 0 and no warnings
(operation `20261015183831.617344254_000027`). It passively read CV 1/RFE 1
from the current-boot Linux driver log. The independent Ethernet production
decoder accepted the actual 256-byte r8169 dump and obtained XID `0x4c0`.
Neither result establishes a working MIND NIC driver.

## Evidence locations and limits

Earlier host/build operation `20261015184244.271928552_000029` completed with
exit 0 on `229d90e8`; it is not evidence for the final candidate. Final x86 chunk
IDs and summaries are recorded with their groups above. The first x2APIC
operation `20261015195815.028731111_000098` stopped in rustup preflight
because DNS could not resolve the update server; no test ran. The unchanged
retry `20261015195949.242223344_000100` passed in 168 s. The preflight failure
is retained and no test or toolchain requirement was relaxed. A second
pre-test DNS failure in wrapper fetch operation `20261015200421.020206465_000107`
prevented the protection group from starting. Its unchanged retry
`20261015200550.926982463_000110` passed in 31 seconds. No failed test is
counted as passed. Redundant wrapper fetches were removed from subsequent
chunks while exact retained-source and clean-tree checks remained. The dedicated final
Ryzen TSC group passed in 47 s (operation `20261015193238.156862195_000072`):
3593 MHz was remeasured as 3294 MHz and the 30-second guest/host monotonic
ratio was 1.000, within the unchanged 3% bound. Private raw evidence stays outside git. Hardware logs and user
notes from F: and E: remain distinct from the later Linux identity observations.
A new MIND boot must still establish the RAM-cache fix's hardware effect, the
new raw-register report, and measured ping/traffic reliability under the
reported workload. Native Effector runtime and real-server acceptance remain
open. External follow-up task 82 records a separate server-ACK/agent-deadline
clock-domain observation: the old i9 operation was terminal on the server
while its process age was still below the local monotonic deadline. Two
read-only samples also measured a realtime/monotonic offset change. The
cause is not established and no production cancellation fix is claimed. A separate PCU explicit-cancellation observation is recorded in external
active task `82-linux-operation-timeout-02-cancel-descendant-groups.task.md`.
Its owned descendants required scoped cleanup, which verified zero survivors.
That task is separate from the clock-domain investigation; its implementation
belongs to the Effector project.

The external ConfigAgent server patch passed its full Go tests locally and
was committed locally on its ASTRA branch as
`50b51630dd27b827b83f8c8ad3532f74293ae271` after the maintainer explicitly
authorized the commit. The Effector owner then integrated it, extended its
server fixtures and committed canonical source `be9d62c`; this is source
integration, not deployment or native MIND acceptance. Active server tasks
81-01/81-02 and 82-01/82-02 remain with the Effector project. Its native-agent
handoff is `issues/requests-EFF.md` in MIND main `bd7b5493`.
