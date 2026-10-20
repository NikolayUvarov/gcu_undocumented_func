# ASTRA targeted x86 integration — 2026-10-16

Task: [213-DRV-0032](../../issues/213-DRV-0032-rtl8852be-wifi.md), with
[213-DRV-0031](../../issues/213-DRV-0031-rtl8168-wired-network.md).
Evidence boundaries: MC-12.1, MC-12.2, MC-12.3 and MC-12.9.

All **nine selected unique groups passed** on the frozen candidate below.
Two additional build prerequisites passed on the second machine. This was
targeted integration validation, **not the full project gate**. It does not
authorize treating the deferred ARM groups, the earlier RAM-above-4-GiB
failure, or physical Realtek networking as passed.

## Exact source and execution

- Tested ASTRA candidate: `c4933c2bca66f0cd0d81bb9a5ca33f8c587a5052`.
- Tested `origin/main` ancestor: `97db4be4c8606a1cd6c21d1895ba8200360bc407`.
- Raw publication acknowledged before these runs:
  `3c6eaa5648c240670dc29fd59d6b17954a01da2f`.
- Own remote branch in both isolated clones:
  `codex/ASR-astra-check-c4933c2b`.
- Both machines checked exact `HEAD`, exact fetched main and main ancestry
  before execution. Main was already an ancestor; the merge did not change
  the tested candidate. Each operation checked exact `HEAD` and the tracked
  and staged diff again before returning.
- Commands used `scripts/ci_local.sh --tree <own-clone> --only host,x86`
  with the exact group selectors in the results table. They did not follow
  a moving ASTRA branch. Later main or ASTRA commits are outside this report.
- Long operations ran through Effector, sequentially on each host, with a
  3600-second ceiling. Independent work on the two hosts overlapped. No
  operation was detached; neither machine was rebooted or had its live NIC
  modified. The final queue check found no remaining operation on either host.
- No build, test or gate ran on `fast-test`. No test assertion was skipped or
  weakened for this aggregate run. Existing explicitly ignored private
  firmware cases remained ignored; proprietary files were not transferred.

The candidate differs from the earlier failed aggregate candidate `d53d9f45`
in test fixtures and documentation, not production source. The corrected
continuous-traffic fixture is included in `c49edca9f2f0989763916fc843561b5d1c5d4e32`.

## Machines and tools

| Property | Ryzen | i9 |
| --- | --- | --- |
| Effector endpoint | `un-rizen_2a8b108d5593` | `i9_4e6882212251` |
| CPU | AMD Ryzen 9 5900HX, 16 logical CPUs | Intel Core i9-9900K, 16 logical CPUs |
| Host OS | Ubuntu 24.04.1 LTS | Ubuntu 24.04.3 LTS under WSL2 |
| Host kernel | `7.0.0-38-generic` | `6.6.87.2-microsoft-standard-WSL2` |
| OpenSSH dependency | Installed `1:9.6p1-3ubuntu13.19` | Privately extracted `1:9.6p1-3ubuntu13.20`; dependency preflight only, no duplicate host-test run |
| Configuration evidence operation | `20261016013027.364733113_000177` | `20261016013205.318448758_000181` |

Both machines reported:

- QEMU 8.2.2, Ubuntu package `1:8.2.2+ds-0ubuntu1.18`.
- OVMF `2024.02-2ubuntu0.10`; `/usr/share/ovmf/OVMF.fd` SHA-256
  `7104ea759145e347d036a48045c38a7c24fb8885b314a9366f61e5b682429186`.
- Pinned toolchain `nightly-2026-10-02-x86_64-unknown-linux-gnu`;
  `rustc 1.101.0-nightly (c36f14571 2026-10-01)`, full compiler commit
  `c36f1457196e315bc204b9564a6a5a7fe7f5a51f`, LLVM 23.1.1.
- OpenJDK package `openjdk-21-jre-headless` version
  `21.0.12.1+1-1~24.04.4`. TLC ran only on Ryzen.

The QEMU commands used the repository's default four CPUs and 512 MiB memory,
except the selected one-CPU group. Individual suite variants remained those
specified by `tests/qemu_smoke.py`; no RAM-6-GiB group was selected.

## Results

Times below are the values reported by `ci_local.sh`, not cross-host
performance measurements. Each listed operation completed with exit code zero.

| Selected group | Host | Result | CI time | Effector operation |
| --- | --- | --- | ---: | --- |
| `build (x86)` | Ryzen | PASS | 1 s | `20261016013027.364733113_000177` |
| `network driver without legacy` | Ryzen | PASS | 5 s | `20261016013027.364733113_000177` |
| `host tests` | Ryzen | PASS | 382 s | `20261016013027.364733113_000177` |
| `models (TLC)` | Ryzen | PASS | 141 s | `20261016013027.364733113_000177` |
| `build (x86 test programs)` | Ryzen | PASS | 44 s | `20261016013027.364733113_000177` |
| `x86: boot, display, network, TLS, shell, memory, clock` | Ryzen | PASS | 840 s | `20261016014100.960957762_000186` |
| `x86: services, storage, audio` | i9 | PASS | 223 s | `20261016013821.102922040_000184` |
| `x86: scheduling, isolation, heap` | i9 | PASS | 118 s | `20261016014236.562352976_000187` |
| `x86: one CPU` | i9 | PASS | 131 s | `20261016014530.851834944_000188` |

The fresh i9 clone also passed `build (x86)` in 222 seconds and
`build (x86 test programs)` in 50 seconds, operation
`20261016013205.318448758_000181`. These are prerequisites repeated on a second
configuration, not two additional unique groups or a second host-test result.

The primary QEMU selector expands to `boot,display,net,tls,normal,memory,dzen`.
The other selectors expand to `services,ahci,audio,tts,listen,hda`,
`busy,smp,isolation,heap`, and `--cpus 1 --suites smp,isolation,heap,services`.
Network/TLS evidence includes virtual-adapter DHCP, ping, DNS, TCP/HTTP,
30-MiB HTTP/HTTPS downloads with interruption and Range resumption, grant and
certificate refusals, and virtual driver/stack restart. These are QEMU results,
not measurements of the installed RTL8852BE or RTL8168.

Independent monotonic and wall-clock snapshots were retained. On Ryzen they
agree to the recorded precision. On i9 they differ: for example, the services
operation measured 211.909 seconds monotonic and 223.413 seconds wall time.
The table preserves CI's reported values; it does not infer device speed or
reinterpret any assertion using those wall-clock differences.

## Retained failures and environment corrections

The earlier aggregate run tested
`d53d9f45a17e55f9883681051d5143e1aacd71d4`, after raw publication
`4a4fa150bc430660efe696d302041a9219c36711`, in operation
`20261016012117.497248190_000173`. Its results remain separate:

| Earlier group | Result | CI time |
| --- | --- | ---: |
| x86 build | PASS | 201 s |
| Driver without legacy | PASS | 5 s |
| Host tests | FAIL | 107 s |
| TLC | PASS | 141 s |
| x86 test programs | PASS | 43 s |

The failing harness had 287 passed, one failed and one inherited private-fixture
ignore. The exact failure was
`data_cases::h2c_runtime::actual_data_owner_wraps_twice_with_receipt_each_time_and_continuous_wifi`
at `tests/rtw89_h2c_data_cases.rs:13`, `assertion failed: c.continuous_wifi > 0`.
All 514 commands completed, but the modeled producer exhausted its finite
stream. The corrected model replaces a Wi-Fi record only at the appropriate
release boundary and checks continuing production and bounded backlog after
every command. Production behavior was unchanged. The earlier failed result
is retained, not relabeled PASS; the complete c493 host group was rerun.

Four i9 setup operations stopped before any build or test execution:

| Operation | Cause and correction |
| --- | --- |
| `20261016012844.373543383_000175` | Explicit `run_as=un` tried `runuser` although the console agent already ran unprivileged as `un`. The retry used that existing account and checked its identity. |
| `20261016012927.622098733_000176` | CI dependency preflight found no `/usr/sbin/sshd`. The documented `MIND_SSHD` override selected a privately extracted official package. |
| `20261016013029.465771010_000178` | The extracted binary lacked `libwrap.so.0`. The official dependency was extracted into the same private directory and selected with operation-local `LD_LIBRARY_PATH`. |
| `20261016013139.015272919_000180` | Metadata collection invoked `rustc` from the dependency directory, outside the repository toolchain override. The retry changed to the own clone before collecting versions. No Rust default was changed. |

The extracted Ubuntu packages were `openssh-server` `1:9.6p1-3ubuntu13.20`
(SHA-256 `5d0d5eb672ddc27601e2da4e8465c5736dc73375a21bc1cd1834fcb08fb155bf`)
and `libwrap0` `7.6.q-33`
(SHA-256 `36118b862539868408ee056411d6e470068d14c46e5296b29bfb744770f5d2ce`).
No system packages, accounts or services were installed or configured.

The successful Ryzen host run generated an untracked
`scripts/hardware/__pycache__/` directory. Only its verified ordinary `.pyc`
files were removed in the own clone before primary QEMU; the directory had
no other contents. The tracked diff remained clean. The subsequent operation
set `PYTHONDONTWRITEBYTECODE=1`. This housekeeping did not change source or
test assertions.

## Evidence locations

These are machine-local retained artifacts, not repository dependencies.
Only this sanitized report is committed; operation JSON and credentials are
not included.

| Host | Own clone | Own TMPDIR |
| --- | --- | --- |
| Ryzen | `/home/un/mind-core/ASR/ASTRA-check-20261016` | `/tmp/mind-astra-check-c493-20261016` |
| i9 | `/home/un/mind-core/ASR/ASTRA-check-20261016-i9` | `/tmp/mind-astra-check-c493-20261016-i9` |

Under the corresponding TMPDIR, full group logs are retained in:

- Ryzen: `logs/01-host/` and `logs/02-primary/`;
  batch consoles `chunk01.console.log` and `chunk02.console.log`.
- i9: `logs/01c-prerequisites/`, `logs/02-services/`,
  `logs/03-scheduling/` and `logs/04-one-cpu/`;
  batch consoles `chunk01c.console.log`, `chunk02.console.log`,
  `chunk03.console.log` and `chunk04.console.log`.
- Earlier d53 Ryzen run: `/tmp/mind-astra-check-20261016/logs/01-host/`,
  `chunk01.console.log` and `failure-01-host/` under that older TMPDIR.

The audit controller retains sanitized summary/manifest files under
`/tmp/mind-astra-private-20261015/`: `c493-integration-summary.md`,
`c493-integration-results.json` and `ryzen-d53-integration-results.json`.
Individual responses are `operation-<Effector-operation-ID>.json` there.
The corrected host operation's stdout begins with a read-only tail of the
preserved d53 failure; its own c493 results appear in the later CI table and
the separate c493 log directory. The old tail is not a new c493 failure.

## Remaining limits

- ARM was deferred by the maintainer and was not run or inferred from x86.
- The previously reported RAM-above-4-GiB failure was not rerun or closed.
- The rest of the full CI matrix was not selected; nine passing groups do not
  constitute the full gate or permission to integrate untested code into main.
- No physical RTL8852BE/RTL8168 association, WPA2 traffic, link recovery,
  latency under load, firmware/radio calibration or Effector session was
  exercised by these runs. Those acceptance criteria remain open.
- Results apply to the stated candidate and configurations. Host models and
  QEMU tests are evidence, not proofs or hardware readiness guarantees.
