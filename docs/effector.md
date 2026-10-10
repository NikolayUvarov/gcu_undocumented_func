# Remote build and test machines through Effector

**Version:** 1.0 (2026-10-10) · **For:** every agent and person working on MIND Core ([AGENTS.md](../AGENTS.md)) · **Constitution:** [v1.6](../constitution/EN/MIND_CORE_Constitution_v1.6.md) MC-12.1, MC-12.9 (evidence names its configuration) · **Russian version:** [effector_RU.md](effector_RU.md) (kept in sync; the English text is the reference)

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
    "timeout_seconds": 3600,             # the execution limit: at most 3600
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

### The rules for working together

1. **Long or CPU-heavy work is sequential.** That means a build, the host tests, a QEMU suite or a gate's group: set `"sequential": true`.
   - All sequential operations for one machine, whichever agent sent them, wait in one first-in, first-out queue, and run one at a time.
   - Set `queue_ttl_seconds` for the queue you see (`GET /api/v1/operations?agent_id=…&status=active`). The wait counts against it, and an operation that waits longer expires.
   - Short read-only checks (versions, free space, a log's tail) are not sequential and run beside the queue.
2. **Nothing outlives its operation.** No `nohup`, `&`, `setsid`, `screen`, `tmux` or `systemd-run` for work. A detached job runs outside the queue and collides with the next agent's build or test.
3. **One operation runs at most an hour** (`timeout_seconds` ≤ 3600). Split longer work into operations that each fit, and queue them one after another: for example, one QEMU group or one build each.
4. **Each track works in its own directory:** `/home/un/mind-core/<TRK>/` (for example `/home/un/mind-core/NET/`), with its own clone and its own `target/` directories.
   - Never build in, clean or change another track's directory.
   - The kernel track's clone is still at `/home/un/mind-core/gcu_undocumented_func` and moves to `KRN/`.
   - The test scripts write fixed paths under `/tmp` (`/tmp/mind-core-*`, `/tmp/mind-*-target`). That is a further reason why builds and QEMU runs must be sequential even in different directories.
5. **Name your operations.**
   - `idempotency_key` starts with your track's code and is unique for each new submission.
   - `reason` names the track, the task and what runs (`"STO 300-STO-0002: the store suite on x86"`). The maintainer reads it on the dashboard.
6. **Retry without duplicating.**
   - A call that was cut off may still have queued its operation. Before you submit again, list the active operations and wait for yours if it is there.
   - When you resubmit an uncertain submission, reuse its `idempotency_key`.
7. **Cancel only your own operation, by its ID** (`POST /api/v1/operations/cancel` with `operation_id`). Never cancel by `agent_id`: that cancels every agent's queued work on the machine.
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
- **Say where it ran:** "local gate on PCU through Effector", and name the configuration of section 1.
- **Until the full gate fits there** (section 1), a remote machine adds evidence beside a gate, and does not replace one.

## 5. When something goes wrong

- **A `timeout` with "agent did not acknowledge"** does not prove that nothing ran. Look at the operation's result and at the machine before you retry.
- **HTTP errors carry `X-Effector-Request-ID`.** Keep it when you report a problem to the maintainer.
- **A machine that does not answer:** tell the maintainer. Do not try to recover it yourself.

## 6. Changes made to the machines

| Date | Machine | Change | By, for |
|---|---|---|---|
| 2026-10-09 | `PCU_585240b00e8a` | Build environment for `un`: `git`, `build-essential`, `curl`, `pkg-config`, QEMU 6.2 (`qemu-system-x86`, `qemu-system-arm`, `qemu-utils`), OVMF, AAVMF, `ipxe-qemu`, `dosfstools`, `mtools`, `swtpm`, `swtpm-tools`, `sbsigntool`, `ffmpeg`; rustup in `~un/.cargo`; the kernel track's clone | KRN, at the maintainer's request |
| 2026-10-10 | `PCU_585240b00e8a` | The Ubuntu Cloud Archive (caracal) added for QEMU 8.2 and removed again: it has no QEMU for 22.04 | KRN, at the maintainer's request |
| 2026-10-10 | `PCU_585240b00e8a` | QEMU 8.2.2 from the qemu.org release tarball, its signature verified (Michael Roth, `CEAC C9E1 5534 EBAB B82D 3FA0 3353 C9CE F108 B584`), built for `x86_64` and `aarch64` with the tools, in `/opt/qemu-8.2.2`, links in `/usr/local/bin`. Its build dependencies from Ubuntu 22.04: `ninja-build`, `python3-venv`, `python3-tomli`, `python3-distlib`, `flex`, `bison`, `libglib2.0-dev`, `zlib1g-dev`, `libpixman-1-dev`, `libslirp-dev`, `libfdt-dev`, `libpng-dev`, `libaio-dev` | KRN, at the maintainer's request (the same QEMU version as CI) |
