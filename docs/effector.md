# Remote build and test machines through Effector

**Version:** 1.3 (2026-10-23): renewing a reservation extends its command's run time, and a run has an hour after the reservation ends (rule 3); 1.2 (2026-10-23): a reservation's lease is no limit; an operation still ends at its `timeout_seconds` until the service stops cutting commands; work in parts of an hour or two, the maintainer's word (rule 3); 1.1 (2026-10-23): 1.0 (2026-10-10) · **For:** every agent and person working on MIND Core ([AGENTS.md](../AGENTS.md)) · **Constitution:** [v1.6](../constitution/EN/MIND_CORE_Constitution_v1.6.md) MC-12.1, MC-12.9 (evidence names its configuration) · **Russian version:** [effector_RU.md](effector_RU.md) (kept in sync; the English text is the reference)

The maintainer lends agents real machines for builds and tests through **Effector**, a service that runs commands on registered computers.

- **Several agents share each machine.** This guide says how to use them without getting in each other's way.
- **The authoritative descriptions of the service** are its own:
  - the agent help, <https://hesere.uz/effector/ahelp>;
  - the API, `GET https://hesere.uz/effector/api/openapi.json` (with the key).
- **Read them first.** Where they and this guide differ, the service's documents win for how to call it, and this guide wins for how MIND's agents share the machines.

## 1. The machines

| Agent ID | What it is | User for work | Use |
|---|---|---|---|
| `PCU_585240b00e8a` | Ubuntu 22.04.5, x86-64, 4 CPUs, 15 GB RAM | `un` | Builds, host tests, QEMU suites, gates. **No reboots.** |
| `i9_4e6882212251` | Ubuntu 24.04.3 under WSL2 on Windows, Intel Core i9-9900K (16 threads), 48 GB RAM for WSL, 920 GB free, `/dev/kvm` | the agent's own, `un`: give **no `run_as`** (the agent is not root, so `runuser` fails) | Builds, host tests, QEMU suites under TCG. **No reboots**: it is WSL2. |
| `DESKTOP-i7-930-wsl-u24` | Ubuntu 24.04.5 under WSL2 on Windows 10 22H2, Intel Core i7-930 (4 cores, 8 threads, 2.8 GHz, SSE4.2, no AVX), 15 GB RAM for WSL, 955 GB free, no `/dev/kvm` | the agent runs as root: give `"run_as": "un"` for work | Builds, host tests, QEMU suites under TCG (x86 and aarch64), with CI's packages; runs that want memory. About 1.6 times i9's time. **No reboots**: it is WSL2. |
| `DESKTOP-i7-930-wsl-u26` | Ubuntu 26.04.1 on the same machine | the agent's own, `un`: give **no `run_as`** | Rust only (rustup, the kernel track's clone): no C linker, QEMU or firmware until `setup_build_env.sh` runs as root there. |
| `un-rizen_2a8b108d5593` (`un-rizen-service`) | Ubuntu 24.04.1 on the machine itself (no WSL; Linux 7.0), AMD Ryzen 9 5900HX (8 cores, 16 threads), 30 GB RAM, 97 GB free on NVMe, `/dev/kvm` | the agent runs as root, as a service: give `"run_as": "un"` for work | Builds, host tests, QEMU suites under TCG (x86 and aarch64), gates: **4 queues for every track and agent** (section 3, rule 1). **No reboots.** |
| `x32-128_f5832ee52625` (`i9-128`) | Ubuntu 22.04.5 on the machine itself (Linux 6.8), Intel Core i9-13900K (24 cores, 32 threads), 125 GB RAM, 91 GB free of 916 GB, `/dev/kvm` | the agent's own, `gater`: give **no `run_as`**. Its commands run in `sh`: start a script with `bash -s`. Clones in `/home/gater/mind-core/<TRK>/`, `TMPDIR=/tmp/mind-<TRK>` | Builds, host tests, QEMU suites and gates (the maintainer, 2026-10-26), once `scripts/setup_build_env.sh` has run there as root: until then it has QEMU 6.2 for x86 only, no AAVMF and no Java. **No reboots.** **Not for gates now (2026-10-26):** gcc crashed while building QEMU at `-j32` (`internal compiler error: Illegal instruction`, segmentation faults in `cc1`), and the machine hung during the next such build and did not boot again. The maintainer checks the hardware; nothing runs there until they say it is stable. |
| `acc-1_e9f12f8b0550` (`R-GATE-77`) | Ubuntu 22.04.3 on the machine itself (Linux 6.8), Intel Core i3-12100 (4 cores, 8 threads), 31 GB RAM, 198 GB free in `/home`, `/dev/kvm` | the agent's own, `un`: give **no `run_as`**. Its commands run in `sh`: no `set -o pipefail`, or start a script with `bash -s`. Clones in `/home/un/mind-core/<TRK>/`, `TMPDIR=/tmp/mind-<TRK>` | Builds, host tests, QEMU suites and gates, x86 and aarch64 (the maintainer, 2026-10-28): `parallel_limit` 2, the landing gate's lane `acc1`. Other accounts live on it: touch none of their files or services. **No reboots.** |
| `acc-2_a01a5832facb` (`R-GATE-78`) | Ubuntu 22.04.3 on the machine itself (Linux 5.15), Intel Core i3-12100 (4 cores, 8 threads), 31 GB RAM, 331 GB free in `/home`, `/dev/kvm` | as on `acc-1` | as on `acc-1`; the landing gate's lane `acc2` |
| `s64_ff4f6639826a` (`R-GATE-S64G`) | Fedora 27 (Linux 4.18), Intel Core i7-8700K (6 cores, 12 threads), 62 GB RAM, 77 GB free in `/home`, `/dev/kvm` | the agent's own, `un`: give no `run_as`; `sudo` asks for a password | **Not for gates now (the maintainer, 2026-10-28):** QEMU 2.10, Python 3.6, git 2.14, no C compiler, no containers. It needs a newer system before `scripts/setup_build_env.sh` (Ubuntu only) can prepare it. |

- **What is installed on `PCU_585240b00e8a`** (and who did it is in section 6):
  - Rust through rustup in `~un/.cargo`, with the toolchain of `rust-toolchain.toml`;
  - **QEMU 8.2.2**, built from the signed qemu.org release, in `/opt/qemu-8.2.2`, first in `PATH` through links in `/usr/local/bin`. The system's QEMU 6.2 stays in `/usr/bin`;
  - from Ubuntu 22.04: OVMF and AAVMF 2022.02, iPXE ROMs, `dosfstools`, `mtools`, `swtpm` 0.6, `sbsigntool`, `ffmpeg`, Python 3.10.
- **How it differs from CI's runners** (Ubuntu 24.04):
  - the firmware: OVMF and AAVMF 2024.02 there;
  - `swtpm` 0.7, Python 3.12 and `python3-virt-firmware` there;
  - QEMU: Ubuntu's patched 8.2.2 there, the plain qemu.org 8.2.2 here.
- **What follows for a run on this machine.** It is evidence for this configuration only (MC-12.1, MC-12.9). Say so where you report it: "PCU through Effector, Ubuntu 22.04, QEMU 8.2.2 (qemu.org), OVMF 2022.02".
- **The full `scripts/ci_local.sh` gate does not run there yet.**
  - Its prerequisites name a Java runtime, and the Secure Boot group needs `python3-virt-firmware`.
  - The whole gate takes longer than one operation may run (section 3).
  - Builds, host tests and single QEMU groups do run there.
  - Two x86 groups go to another machine, as measured on 2026-10-14 (the tools track's gate of eda1c74):
    - "x86: Secure Boot with our keys" stops at once with `No module named 'virt'`;
    - "x86: reproducible build" builds the image twice and ran past the hour its operation was given. It took 7 minutes on i9 and 26 on `DESKTOP-i7-930-wsl-u24`.
    - Send them to a machine with CI's packages: `i9`, `un-rizen-service` or `DESKTOP-i7-930-wsl-u24`.

- **What is on `i9_4e6882212251`** (section 6):
  - everything [`scripts/setup_build_env.sh`](../scripts/setup_build_env.sh) installs, from Ubuntu 24.04:
    - QEMU 8.2.2 for x86 and aarch64, OVMF and AAVMF 2024.02, iPXE ROMs;
    - `dosfstools`, `mtools`, `swtpm` 0.7.3;
    - `sbsigntool`, `python3-virt-firmware` 24.1;
    - a Java runtime;
  - `git`, Python 3.12;
  - Rust through rustup in `~un/.cargo`, with the toolchain of `rust-toolchain.toml`;
  - the kernel track's clone in `/home/un/mind-core/KRN/gcu_undocumented_func`.
- **Its packages are CI's** (Ubuntu 24.04), unlike PCU's. Say so where you report a run: "i9 through Effector, Ubuntu 24.04 under WSL2, QEMU 8.2.2 (Ubuntu), TCG".
- **WSL's settings** (`%UserProfile%\.wslconfig`, the maintainer's): 48 GB of memory, all 16 threads, nested virtualization on.
- **KVM.** `/dev/kvm` is there and `un` is in the `kvm` group, but the suites run under TCG, as in CI: none asks for KVM.
- **A proxy.** The agent runs with `HTTPS_PROXY` and `HTTP_PROXY` set and no `NO_PROXY`. A test that serves on `127.0.0.1` is then reached through the proxy and fails: `release_test.py`'s HTTPS server failed the host tests with "Tunnel connection failed: 503". Start an operation with `export NO_PROXY=127.0.0.1,localhost,::1 no_proxy=127.0.0.1,localhost,::1`.
- **Measured on 2026-10-11** (762907d merged with `main` b0ca713, one operation):

  | Group | i9 | The kernel session's machine |
  |---|---|---|
  | build (x86) | 149 s | 240 s |
  | build (x86 test programs) | 31 s | 28 s |
  | x86: boot, display, network, TLS, shell, memory, clock | 952 s | 784 s |

  All three passed. The builds are faster there. The QEMU suites are not, as they mostly wait inside the guest.
- **What is on `un-rizen_2a8b108d5593`**, the maintainer's Ryzen machine, added on 2026-10-14 (section 6):
  - everything `scripts/setup_build_env.sh` installs, from Ubuntu 24.04, as on i9: QEMU 8.2.2 for x86 and aarch64, OVMF and AAVMF 2024.02, iPXE ROMs, `dosfstools`, `mtools`, `swtpm` 0.7.3, `sbsigntool`, `python3-virt-firmware` 24.1, OpenJDK 21, `build-essential` (gcc 13.3), `git`;
  - Rust through rustup in `~un/.cargo`, with the toolchain of `rust-toolchain.toml`; `un` is in the `kvm` group;
  - the kernel track's clone in `/home/un/mind-core/KRN/gcu_undocumented_func`. Another track clones its own into `/home/un/mind-core/<TRK>/` as `un` the first time (rule 4): the toolchain in `~un/.rustup` is shared, and nothing needs installing.
- **Its packages are CI's** (Ubuntu 24.04), and no proxy is set for the agent. Say so where you report a run: "un-rizen-service through Effector, Ubuntu 24.04 (native), QEMU 8.2.2 (Ubuntu), TCG".
- **Its first agent,** `un-rizen_f5809c60f795`, ran in a console; the maintainer moved it to a service on 2026-10-14. Send nothing to the old ID: an operation there does not run.
- **Measured on 2026-10-14** (the kernel track's e2e1133 merged with `main` 39f5199, one group to an ordinary operation, the track's two lanes at once, seconds):

  | Part | Groups and times |
  |---|---|
  | aarch64, 51 min in all | build 185; boot and fault containment 93; programs, shell and four CPUs 567; files, network and TLS 1026; RAM, ACPI and PCI above 4 GiB 467; GICv2 410; NVMe 102; 16 CPUs 220 |
  | x86, 87 min in all | build 325; test programs 34; services, storage, audio 185; scheduling, isolation, heap 97; NVMe 170; 16 CPUs 113; RAM above 4 GiB 468; one CPU 113; AVX 33 and 33; no PIT 120; x2APIC 168; padded vector area 73; protection probes 32; TSC 47; USB image 82; Secure Boot 20; reproducible build 355; boot, display, network, TLS 717; keys, shell, tools 1267; files and block writes 791 |

  All passed. Three x86 groups needed a second run, below.
- **Two of one track's QEMU groups at once can fail the short waits.** On 2026-10-14 the kernel track ran two lanes. These three groups failed while the other lane ran a QEMU group or the reproducible build, and passed when run again:
  - "boot, display, network, TLS": `run tlsclock &` waits 8 s for the prompt;
  - "keys, shell, tools";
  - "files and block writes".

  Run those three with no second group of your own beside them. Rerun one that failed under load once, with nothing of yours beside it, before you call it a fault in the code.
- **What is on `acc-1_e9f12f8b0550` and `acc-2_a01a5832facb`**, the maintainer's gate machines of 2026-10-28 (section 6):
  - everything `scripts/setup_build_env.sh` installs, from Ubuntu 22.04, as on PCU: QEMU 8.2.2 built from the signed qemu.org release in `/opt/qemu-8.2.2` (first in `PATH` through `/usr/local/bin`; Ubuntu's 6.2 stays in `/usr/bin`), OVMF and AAVMF 2022.02, `swtpm` 0.6.3, `sbsigntool` 0.9.4, `python3-virt-firmware` 24.1.1, OpenJDK 11, `build-essential`, Python 3.10;
  - Rust through rustup in `~un/.cargo`, with the toolchain of `rust-toolchain.toml`; `un` is in the `kvm` group from the agent's next start, and the suites run under TCG anyway;
  - the kernel track's clone in `/home/un/mind-core/KRN/gcu_undocumented_func`. Another track clones its own into `/home/un/mind-core/<TRK>/` as `un` (rule 4): the toolchain in `~un/.rustup` is shared.
- **Their packages are PCU's, not CI's** (Ubuntu 22.04: the firmware 2022.02, `swtpm` 0.6, Python 3.10, the plain qemu.org QEMU). Say so where you report a run: "acc-1 through Effector, Ubuntu 22.04, QEMU 8.2.2 (qemu.org), OVMF 2022.02, TCG".
- **Measured on 2026-10-28** (`main` 0e006ca2, one lane on each machine, seconds; beside them un-rizen's: round 39's for the x86 groups, those of 2026-10-14 for the builds and aarch64):

  | Group | `acc-1` or `acc-2` | un-rizen |
  |---|---|---|
  | build (x86) | 311 (acc-1) | 325 |
  | build (x86 test programs) | 61 (acc-1) | 34 |
  | build (aarch64) | 392 (acc-2) | 185 |
  | x86: one CPU | 121 | 127 |
  | x86: no PIT | 122 | 128 |
  | x86: x2APIC | 182 | 189 |
  | x86: scheduling, isolation, heap | 111 | 112 |
  | aarch64: boot and fault containment | 173 (acc-2) | 93 |
  | aarch64: NVMe boot disk | 82 (acc-2) | 102 |
  | x86: keys, shell, tools | 1010 (acc-1) | 1047 |
  | x86: files and block writes | 1047 (acc-2) | 1032 |
  | x86: services, storage, audio | 561 (acc-2) | 541 |
  | x86: RAM above 4 GiB | 1092 (acc-2) | 1079 |
  | x86: reproducible build | 724 (acc-2) | 509 |

  All these passed. The long x86 groups take about as long as on un-rizen, one lane on each machine.
- **What fails there, because of Python 3.10:**
  - the host tests: `scripts/models.py` and `tests/models_host.rs` read TOML with `tomllib` (Python 3.11). `scripts/ci_local.sh` now names it among the missing tools for the host part. Run the host part on un-rizen or i9;
  - the Effector suites: `tests/effector_server.py` answers 400 to every POST without a query, as Python 3.10's `parse_qs` refuses an empty query in strict mode (`issues/requests-EFF.md`). The groups with them, x86 "boot, display, network, TLS, shell, memory, clock" and aarch64 "files, network and TLS", run elsewhere until EFF's fix.
- **Setting up another machine.** `scripts/setup_build_env.sh` installs everything a build and `scripts/ci_local.sh` need, and checks it. It is idempotent: what is there is left alone. Run it once as root, for example from Windows: `wsl -u root -- bash scripts/setup_build_env.sh USER CLONE`.

## 2. The key

- **Getting a key.** Ask the maintainer for an API key for your session. Keep it only in a file in your session's private directory (mode 0600), never in the repository.
- **How to send it.** Send it only in the `X-API-Key` header.
- **Where it must never appear:** a URL, a command line (where the process list shows it), a log, a commit, an issue or a reply. Do not print it to check it: `GET /api/v1/api-key` tells you whether it works.
- **A key per session**, so that the maintainer sees whose operation is whose.

A minimal client in Python (the standard library only):

```python
import json, os, time, urllib.request

BASE = "https://hesere.uz/effector"
KEY = open(os.environ["EFFECTOR_KEY_FILE"]).read().strip()  # a 0600 file in your private directory

def call(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    request = urllib.request.Request(BASE + path, data=data, method=method,
                                     headers={"X-API-Key": KEY, "Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=60) as reply:
        return json.load(reply)
```

## 3. Submitting work

An operation runs one shell script on a machine as a user:

```python
script = open("build_and_test.sh").read()   # POSIX sh; keep its output short (tail your logs)
op = call("POST", "/api/v1/operations", {
    "agent_id": "PCU_585240b00e8a", "kind": "exec", "run_as": "un", "command": script,
    "timeout_seconds": 3600,             # this operation's own deadline, chosen by its sender (the API takes 1 to 3600)
    "sequential": True,                  # long or CPU-heavy work waits in the machine's queue
    "queue_ttl_seconds": 6 * 3600,       # how long it may wait behind other agents' work
    "idempotency_key": "KRN-351-KRN-0057-x86-20261010T0400",
    "reason": "KRN 351-KRN-0057: the x86 group on QEMU 8.2.2"})
FINAL = {"completed", "failed", "timeout", "expired", "cancelled", "rollback",
         "run_as_unavailable", "interactive_session_unavailable", "denied"}
while op["status"] not in FINAL:
    time.sleep(5)
    op = call("GET", f"/api/v1/operations?id={op['id']}")
    # while queued: op["queue_position"], 1 is next to start
print(op["status"], op.get("result", {}).get("exit_code"), op.get("result", {}).get("stdout", "")[-3000:])
```

### Choosing a machine (the maintainer, 2026-10-14)

Machines join, leave and change: the maintainer adds them and may move an agent to a service (its ID then changes), and other tracks fill their queues. So choose the machine afresh:

- **When.** Before each batch of long work (a build, a suite, a gate's part), and again when a batch waits in a queue or runs much slower than it used to.
- **How.**
  1. `GET /api/v1/endpoints`: each machine's `online`, `parallel_limit`, `occupied_queues` and `free_slots`.
  2. `GET /api/v1/operations?agent_id=…&status=active` for the candidates: whose work is queued or running there, and for how long.
  3. Section 1: what each machine has (the aarch64 tools, CI's packages or others), the user to give, and how fast it is.
- **Which.** The machine that has what the work needs and the most room for it:
  - a free queue first: work queued behind other tracks' costs more than a slower machine that is free;
  - among the free ones, the faster: today `un-rizen-service` (4 queues), then `acc-1` and `acc-2` (2 queues each, x86 groups as fast as un-rizen's), then `i9` (5 queues, but shared and under WSL), then PCU (its QEMU work goes sequential), then `DESKTOP-i7-930-wsl-u24`;
  - a machine already loaded with other tracks' QEMU groups only when nothing else is free: under load a group runs past its timeouts, and its failure says nothing of the change (on 2026-10-14 i9 at a load of 39 took over an hour on a group that takes 16 minutes);
  - the parts of a gate may run on different machines, and every group must pass on the same commit.
- **A machine missing from section 1** has a configuration nobody described: describe it there, with the maintainer, before its runs count as evidence.
- **Your own operation that waits long** may be cancelled by its ID (rule 7) and sent where there is room.
- **The report** says where each part ran (section 4).

### The rules for working together

1. **Long or CPU-heavy work runs one operation at a time for each track.** That means a build, the host tests, a QEMU suite or a gate's group.
   - A machine runs as many operations at once as it has queues: `parallel_limit` in `GET /api/v1/endpoints`, with `occupied_queues` and `free_slots`. The maintainer sets it on the Agents page by the machine's cores: two QEMU groups at once need about twice the threads, and a 16-CPU group takes 16.
   - **One queue** (`parallel_limit` 1): set `"sequential": true`. All sequential operations for one machine, whichever agent sent them, wait in one first-in, first-out queue and run one at a time.
   - **PCU** (4 CPUs) has had `parallel_limit` 3 since 2026-10-12, but its QEMU groups and builds still go sequential. On 2026-10-12 two tracks' groups ran there at once as ordinary operations: the load rose to 15–20 and the kernel track's aarch64 groups timed out. Its other queues are for short, light work.
   - **`un-rizen-service`** (16 threads) has `parallel_limit` 4, **for every track and agent** (the maintainer, 2026-10-14). Use it for builds, host tests, QEMU groups and gates, x86 and aarch64:
     - send long work there as ordinary operations, never `"sequential"`: the sequential lane is one queue and would leave three idle;
     - each track runs one long operation there at a time, so four tracks build and test at once;
     - a track may take a second queue only while two are free, from a second clone and a `TMPDIR` of its own (`/home/un/mind-core/<TRK>-2/`, `/tmp/mind-<TRK>-2`): two groups in one clone or one `TMPDIR` overwrite each other's images and logs;
     - a QEMU group takes about as many threads as its guest has CPUs (four for most groups), so four fit; a 16-CPU group takes the whole machine while it runs, and the other groups then go slower.
     - x86's "boot, display, network, TLS", "keys, shell, tools" and "files and block writes" go in your first lane only, with no second group of yours beside them: two at once failed their short waits (section 1, measured on 2026-10-14).
   - **More queues:** send the long work as ordinary operations (no `"sequential"`), one at a time: the next only when the last has ended. Work above the limit waits for a free queue. Sequential operations keep sharing one lane, so a track that marks its work sequential waits behind the others' sequential work.
   - Every track keeps its temporary files in its own `TMPDIR` (rule 4), whatever the queue.
   - Set `queue_ttl_seconds` for the queue you see (`GET /api/v1/operations?agent_id=…&status=active`). The wait counts against it, and an operation that waits longer expires.
   - Short read-only checks (versions, free space, a log's tail) are not sequential. They still take a queue: with one queue they wait for the running operation.
2. **Nothing outlives its operation.** No `nohup`, `&`, `setsid`, `screen`, `tmux` or `systemd-run` for work. A detached job runs outside the queue and collides with the next agent's build or test.
3. **Time** (the maintainer, 2026-10-23).
   - **A queue reservation's 1 to 60 minutes is a lease, not a limit on the work.** It is there so that a reservation someone forgot frees itself. Renew it as often as the work needs: the service limits neither the total time nor the number of renewals.
   - **Renewing a reservation extends the run time of the command sent under it** (with the reservation's session and PIN headers, the service's help), and after the reservation ends the run still has an hour to finish. So a long gate held under a reservation you renew is not cut.
   - **An operation sent without a reservation ends at its `timeout_seconds`** (the API takes 1 to 3600): the server ends it with `agent did not acknowledge the operation before timeout_seconds elapsed`.
   - **Split the work into parts of about an hour, two at most,** all the same: for example one QEMU group or one build each, queued one after another. On a slow machine a group that takes longer goes to a faster one, or into smaller parts.
   - **A `timeout` says the part outran its time, not that the change is wrong.** Report it as such: which `timeout_seconds` the operation had and what it had done by then.
4. **Each track works in its own directory:** `/home/un/mind-core/<TRK>/` (for example `/home/un/mind-core/NET/`), with its own clone and its own `target/` directories.
   - Never build in, clean or change another track's directory.
   - The kernel track's clone is still at `/home/un/mind-core/gcu_undocumented_func` and moves to `KRN/`.
   - **Each operation exports `TMPDIR=/tmp/mind-<TRK>`** (`mkdir -p` it first). `scripts/ci_local.sh`, the test programs, the kernel variants, the logs and the QEMU suites' files then go there ([000-KRN-0092](../issues-done/000-KRN-0092-two-tracks-test-side-by-side.done)), so two tracks' runs on one machine share no file and no lock. Keep the path short: the suites put QEMU's control sockets under it, and a Unix socket's path must stay under 108 bytes. A test or script that still writes a fixed path under `/tmp` is a bug: report it to the track that owns it.
5. **Name your operations.**
   - `idempotency_key` starts with your track's code and is unique for each new submission.
   - `reason` names the track, the task and what runs (`"STO 300-STO-0002: the store suite on x86"`). The maintainer reads it on the dashboard.
6. **Retry without duplicating.**
   - A call that was cut off may still have queued its operation. Before you submit again, list the active operations and wait for yours if it is there.
   - When you resubmit an uncertain submission, reuse its `idempotency_key`.
7. **Cancel only your own operation, by its ID**: `POST /api/v1/operations/cancel` with `operation_id`, an `idempotency_key` of its own and a `reason`; without the last two the request is refused (HTTP 400). Queued and running operations can be cancelled. Never cancel by `agent_id`: that cancels every agent's queued work on the machine.
8. **Work as `un`.**
   - The root account (`run_as` `service`) is for a change to the machine that the maintainer asked for. Record such a change in section 6 in the same commit as the work that needed it.
   - Never install, remove or upgrade packages, change `/opt`, `/usr/local` or the system's configuration for your own convenience. Ask the maintainer.
9. **Never reboot or shut a machine down,** and never touch:
   - its network configuration;
   - its users, `ssh`, or the service that runs Effector's agent;
   - its other services, or other users' files.

   A machine without its network is out of reach until someone stands in front of it.
10. **`fast-test` is never built or tested here either** ([AGENTS.md](../AGENTS.md), section 4): only your own branch.
11. **Clean up after yourself.** Remove your own large outputs when done. Look at `df -h /home` before a large build.

## 4. Gates on a remote machine

- **A gate run through Effector is a local gate** in the sense of [AGENTS.md](../AGENTS.md), section 4, when every group of `scripts/ci_local.sh` passes.
- **The landing queue's gate** (AGENTS.md, section 4, since 2026-10-21) runs this way. `scripts/land.sh gate` spreads every group of `scripts/ci_local.sh --list` over the machines' lanes and runs a failed group once more, alone. It sends the key only from the file `$EFFECTOR_KEY_FILE` (section 2). The presets name its lanes: `full`, and `no-i9` while another track's gate loads i9; `i9-on-rz`, which rounds 34 to 39 used while i9 was offline; `wide` since 2026-10-28, with `acc-1` and `acc-2` taking the long x86 groups, and `wide-no-i9` while i9 is busy.
- **The pool** (`--preset pool`, or `pool-no-i9` while i9 is busy; since 2026-10-29, the maintainer's ask). No lane holds a list.
  - **The lanes:** i9, `acc-1`, `acc-2`, two clones on `un-rizen-service` and PCU. A third clone there slowed the groups by a third and failed `kbench`'s limit for a call and reply (round 44).
  - **What each takes:** as it frees, the longest group left that it can run. The host part runs on `un-rizen-service` only, i9 takes no aarch64 group, and PCU takes only aarch64 groups.
  - **Builds:** a lane builds a part's image the first time it takes one of its groups.
  - **The estimates** are the seconds below, then the gate's own records of the same groups; `--dry-run` prints the schedule they give.
  - **A gate started again** on the same candidate gives each lane the groups it took before, in the same order (`$LAND_WORK/gate-<commit>-taken.json`), so Effector answers with the results it holds.
  - **A machine that is off** gives no lane: the gate asks Effector which machines are online as it starts and logs the others. A lane's tree that waits half an hour in its queue leaves that lane out too (round 47: `acc-1` and `acc-2` went off at 20:07, and their trees waited in the queue).
- **The long groups are split** (2026-10-29) so that none runs much past 15 minutes. Each suite was timed alone on `3e07edb8`, in seconds:

| Group before | Its suites alone | The groups now |
|---|---|---|
| x86: boot, display, network, TLS, shell, memory, clock (1489, `un-rizen-service`) | boot 432, display 56, net 545, tls 101, effector 193, normal 100, memory 23, dzen 39 | x86: boot, display, shell, memory, clock; x86: network, TLS and Effector |
| x86: keys, shell, tools (1023, `acc-1`) | wm 662; keys, shell, tools, bench, windows, tablet, usb, ehci 361 together | x86: keys, shell, tools; x86: the window manager |
| x86: files and block writes (1091, `acc-2`) | vfs 120, edit 24, disk 29, block 20, store 146, storefaults 58, updater 174, linkupdate 520 | x86: files and block writes; x86: the updater and an update through the link |
| aarch64: files, network and TLS (2640, `un-rizen-service`) | vfs 151, store 164, storefaults 65, net 482, tls 98, effector 209, updater 371, linkupdate 899, modes 201 | aarch64: files and the block store; aarch64: network, TLS and Effector; aarch64: the updater and the modes; aarch64: an update through the link |
| aarch64: RAM, ACPI and PCI above 4 GiB (1394, PCU) | normal 476, net 550, fill 368 | aarch64: RAM, ACPI and PCI above 4 GiB; aarch64: network above 4 GiB |

- **What it gives,** in simulation with these estimates: about 56 minutes with `pool`'s six lanes and 63 with `pool-no-i9`'s five, against 64 to 70 for rounds 41 to 43 with fixed lists (49 with a third lane on `un-rizen-service`, which its timing tests did not bear). The tests take about 233 minutes in all, and each lane builds its images first. More lanes are the next step, such as `x32-128` once it is stable.
- **What it gave** in round 44 (2026-10-29, candidate e6678d4a, `pool`): every group once in 41.5 minutes, the builds included; with the one group that failed run again alone, 48.5.
- **Say where it ran:** "local gate on PCU through Effector", and name the configuration of section 1.
- **Until the full gate fits there** (section 1), a remote machine adds evidence beside a gate, and does not replace one.

### The aarch64 part of a gate runs on a remote machine (the maintainer's decision, 2026-10-10)

- **Where.** On a machine of section 1 that has the aarch64 tools, chosen when the part starts (section 3, "Choosing a machine"): `PCU_585240b00e8a` and, since 2026-10-14, `un-rizen-service`, which is faster and has four queues (section 3, rule 1). The maintainer is adding more machines; each comes into section 1 with its configuration.
- **Why.** On an agent's own machine the x86 part of the local gate takes about 70 minutes and the aarch64 part about 65 more. PCU runs the aarch64 groups 1.2–1.6 times slower, but **in parallel** with the x86 part. A gate then takes about as long as its longer half, and the agent's machine does only the x86 part.
- **Measured on 2026-10-10** (the kernel track's tree 9c843b4, one group to an operation, seconds):

| Group | The kernel session's machine | PCU |
|---|---|---|
| build (aarch64) | 214 s | 248 s |
| aarch64: boot and fault containment | 90 s | 8 s, failed: QEMU aborted in `vvfat` (106 s and passing since 000-KRN-0067) |
| aarch64: programs, shell and four CPUs | 339 s | 394 s |
| aarch64: files, network and TLS | 1121 s | 1819 s |
| aarch64: RAM, ACPI and PCI above 4 GiB | 826 s | 1063 s |
| aarch64: GICv2 with GICv2m | 758 s | 922 s |
| aarch64: NVMe boot disk | 132 s | 187 s |
| aarch64: 16 CPUs | 340 s | 525 s |

- **How.** Run it in your track's clone, `/home/un/mind-core/<TRK>/gcu_undocumented_func`, with one sequential operation (`"sequential": true`) per step, each well under the hour an operation may run:
  1. **The tree.** Bring the clone to your branch merged with `main`, as `--ref` would:
     - `git merge --abort; git reset -q --hard`;
     - `git fetch origin`, `git checkout --detach origin/<your branch>`;
     - `git merge --no-edit origin/main`.
  2. **The build.** `scripts/ci_local.sh --only aarch64 --group "build (aarch64)"`.
  3. **The groups.** Each group of the aarch64 part on that same tree, one operation each: `scripts/ci_local.sh --only aarch64 --group "aarch64: programs, shell and four CPUs"`, and so on. `scripts/ci_local.sh --list --only aarch64` names them.
  4. **The logs.** Copy what you need into your track's directory in the same operation. The next run of anyone's `ci_local.sh` removes `/tmp/mind-core-*.log` and rewrites `/tmp/mind-ci-local`.
- **The report** says where each part ran: "x86 part local; aarch64 part on PCU through Effector (Ubuntu 22.04, QEMU 8.2.x from qemu.org, AAVMF 2022.02)". Together they are one local gate in the sense of [AGENTS.md](../AGENTS.md), section 4, when every group of both parts passed on the same commit.
- **In all, about 86 minutes on PCU** against 64 on the kernel session's machine, for the groups that pass.
- **"aarch64: boot and fault containment" runs on PCU too** (since 2026-10-11, [000-KRN-0067](../issues-done/000-KRN-0067-the-aarch64-boot-test-on-pcu.done)).
  - It used to fail there: QEMU's `vvfat` driver aborted as the guest wrote its boot volume (`fat:rw:`), with `handle_renames_and_mkdirs: Assertion 'j < s->mapping.next' failed`.
  - The test now boots raw FAT images of the build, and `vvfat` serves only a read-only decoy disk. It passed on PCU in 106 s (cf0283f) and on the kernel session's machine.
  - Run it in the aarch64 part with the other groups (step 3).

## 5. When something goes wrong

- **A `timeout` with "agent did not acknowledge"** does not prove that nothing ran. Look at the operation's result and at the machine before you retry.
- **HTTP errors carry `X-Effector-Request-ID`.** Keep it when you report a problem to the maintainer.
- **A machine that does not answer:** tell the maintainer. Do not try to recover it yourself.

## 6. Changes made to the machines

| Date | Machine | Change | By, for |
| 2026-10-25 | `un-rizen_2a8b108d5593` | **A permission, no change made yet.** The kernel session may switch this machine to MIND Core: root only to set `BootNext` to the MIND Core stick with `scripts/boot_switch.py` (000-KRN-0124), then the reboot; the next boot returns to Linux. Only once the maintainer says the stick and `effector485` are ready. The second NVMe disk (04:00.0, someone else's Windows disk) is never written. Each run gets a row of its own here | the maintainer's word, for 213 (the network first) |
| 2026-10-26 | `x32-128_f5832ee52625` | As `gater`, nothing as root: the kernel track's clone in `/home/gater/mind-core/KRN/gcu_undocumented_func`; rustup 1.29.2 in `~gater/.cargo` with the toolchain of `rust-toolchain.toml` (nightly-2026-10-02); `scripts/setup_build_env.sh` of e17e215b copied to `/home/gater/mind-core/setup_build_env.sh` (SHA-256 `acdc2543…`) for the maintainer to run as root: `sudo bash /home/gater/mind-core/setup_build_env.sh gater /home/gater/mind-core/KRN/gcu_undocumented_func`. It installs the missing packages (`qemu-system-arm`, `qemu-efi-aarch64`, `default-jre-headless`, `python3-virt-firmware` 24.1.1, QEMU's build dependencies), builds QEMU 8.2.2 from the signed qemu.org release into `/opt/qemu-8.2.2` (the signature check was tried as `gater` first: good) and adds `gater` to `kvm` | KRN, at the maintainer's request (gates on this machine) |
| 2026-10-28 | `acc-1_e9f12f8b0550`, `acc-2_a01a5832facb` | As `un`: the kernel track's clone in `/home/un/mind-core/KRN/gcu_undocumented_func`; rustup in `~un/.cargo` with the toolchain of `rust-toolchain.toml` (nightly-2026-10-02). As root, through `un`'s `sudo`: `scripts/setup_build_env.sh un <the clone>` of `main` 0e006ca2 (SHA-256 `39572de5…`), with `NEEDRESTART_SUSPEND=1` so that no service restarted. It installed `build-essential`, `pkg-config`, `qemu-system-arm`, `ovmf`, `qemu-efi-aarch64`, `swtpm`, `swtpm-tools`, `default-jre-headless`, `python3-virt-firmware` and QEMU's build dependencies, built QEMU 8.2.2 from the signed qemu.org release into `/opt/qemu-8.2.2` with links in `/usr/local/bin`, and added `un` to `kvm` | KRN, at the maintainer's request (gates on these machines) |
| 2026-10-28 | `s64_ff4f6639826a` | Nothing: read-only questions (versions, free space) | KRN; out of gates until its system is newer (the maintainer) |
|---|---|---|---|
| 2026-10-09 | `PCU_585240b00e8a` | Build environment for `un`: `git`, `build-essential`, `curl`, `pkg-config`, QEMU 6.2 (`qemu-system-x86`, `qemu-system-arm`, `qemu-utils`), OVMF, AAVMF, `ipxe-qemu`, `dosfstools`, `mtools`, `swtpm`, `swtpm-tools`, `sbsigntool`, `ffmpeg`; rustup in `~un/.cargo`; the kernel track's clone | KRN, at the maintainer's request |
| 2026-10-10 | `PCU_585240b00e8a` | The Ubuntu Cloud Archive (caracal) added for QEMU 8.2 and removed again: it has no QEMU for 22.04 | KRN, at the maintainer's request |
| 2026-10-10 | `PCU_585240b00e8a` | QEMU 8.2.2 from the qemu.org release tarball, its signature verified (Michael Roth, `CEAC C9E1 5534 EBAB B82D 3FA0 3353 C9CE F108 B584`), built for `x86_64` and `aarch64` with the tools, in `/opt/qemu-8.2.2`, links in `/usr/local/bin`. Its build dependencies from Ubuntu 22.04: `ninja-build`, `python3-venv`, `python3-tomli`, `python3-distlib`, `flex`, `bison`, `libglib2.0-dev`, `zlib1g-dev`, `libpixman-1-dev`, `libslirp-dev`, `libfdt-dev`, `libpng-dev`, `libaio-dev` | KRN, at the maintainer's request (the same QEMU version as CI) |
| 2026-10-10 | `PCU_585240b00e8a` | QEMU 8.2.10 from the qemu.org release (the same signer) built in `~un/qemu-upstream/stage` to try the aarch64 boot test (000-KRN-0067), **not installed**; Ubuntu noble's QEMU patch list in `~un/qemu-noble-patches` (read only) | KRN |
| 2026-10-11 | `i9_4e6882212251` | rustup 1.29.1 in `~un/.cargo` (no change to the shell's profile: `. ~/.cargo/env`) with the toolchain of `rust-toolchain.toml` (nightly-2026-10-02, its four targets); the kernel track's clone in `/home/un/mind-core/KRN/gcu_undocumented_func`; `/home/un/mind-core/setup_build_env.sh`, a copy of `scripts/setup_build_env.sh`, not run yet (`sudo` asks for a password) | KRN, at the maintainer's request |
| 2026-10-11 | `i9_4e6882212251` | `setup_build_env.sh` run as root: Ubuntu 24.04's QEMU 8.2.2 (`qemu-system-x86`, `qemu-system-arm`, `qemu-utils`), `ovmf` and `qemu-efi-aarch64` 2024.02, `ipxe-qemu`, `dosfstools`, `mtools`, `swtpm`, `swtpm-tools`, `sbsigntool`, `python3-virt-firmware`, `un` in the `kvm` group; WSL given 48 GB and nested virtualization, then restarted | the maintainer |
| 2026-10-12 | `DESKTOP-3CNRULI_075845e7423d`, now `DESKTOP-i7-930-wsl-u26` | rustup in `~un/.cargo` with the toolchain of `rust-toolchain.toml` (nightly-2026-10-02); the kernel track's clone in `/home/un/mind-core/KRN/gcu_undocumented_func`. A first build stopped at `linker cc not found`: `scripts/setup_build_env.sh` (in that clone) is to be run as root, `sudo` asks for a password | KRN, at the maintainer's request |
| 2026-10-12 | `DESKTOP-i7-930-wsl-u24` | The kernel track's clone in `/home/un/mind-core/KRN/gcu_undocumented_func`; `scripts/setup_build_env.sh un` run as root (the agent's own user): Ubuntu 24.04's QEMU 8.2.2, OVMF and AAVMF 2024.02, `ipxe-qemu`, `dosfstools`, `mtools`, `swtpm`, `sbsigntool`, `python3-virt-firmware`, `build-essential`, a Java runtime (OpenJDK 21), rustup and the pinned toolchain for `un`. Measured on `main` e028538: `02_build.sh` from nothing 710 s, the test programs 72 s, "x86: scheduling, isolation, heap" 180 s (i9: 111 s). No nested virtualization: Windows 10 has none for WSL2, and the guest sees no VMX | KRN, at the maintainer's request |
| 2026-10-14 | `un-rizen_f5809c60f795`, now `un-rizen_2a8b108d5593` (`un-rizen-service`) | `git` and `ca-certificates`; the kernel track's clone in `/home/un/mind-core/KRN/gcu_undocumented_func`; `scripts/setup_build_env.sh un` run as root (138 s): Ubuntu 24.04's QEMU 8.2.2, OVMF and AAVMF 2024.02, `ipxe-qemu`, `dosfstools`, `mtools`, `swtpm` 0.7.3, `sbsigntool`, `python3-virt-firmware` 24.1, `build-essential`, OpenJDK 21, rustup and the pinned toolchain for `un`, `un` in the `kvm` group. The maintainer then moved the agent from a console to a service with 4 queues | KRN, at the maintainer's request |
