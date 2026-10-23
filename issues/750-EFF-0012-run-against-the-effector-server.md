# 750-EFF-0012 — A run against the real Effector server, recorded per configuration

**Type:** evidence · **Owner:** `EFF` track · **Priority:** P2 · **Status:** open · **Blocked by:** 0006–0008; deployment and agent/live acceptance in `sst-test-deploy: issues/81-mindcore-01-agent-and-live-acceptance.task.md`, and its `issues/81-mindcore-02-package-metadata.task.md` for the signed-package checks; [issues-human](../issues-human/README.md#7-an-effector-server-for-mind-core) (a server to run against); [550](550-network-on-real-hardware.md) for the real-hardware part · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-12.1, MC-12.2, MC-12.9

## Problem

The test server of 0002 follows the contract as read. Compatibility with the real server is a separate claim, and it holds only for the server builds and the configurations it was checked on (MC-12.9).

The local server-only platform boundary is recorded in external `issues-done/81-mindcore-server-platform-boundary.task.md.done`. Its prepared change has not been deployed; it neither implements a MIND runtime agent nor closes the package metadata/signature requirements of 0008. Historical external task 75 is unrelated console work and is not a dependency of this run.

## Plan

- **In QEMU against a real Effector server** at a named build, on the maintainer's machine or a test server:
  - the endpoint appears in the server's web interface and API with `os_version` `mindcore`;
  - from the server's interface and its operator API:
    - restart a service, collect a log and a file, deliver a file;
    - install, update and remove a signed package;
    - check for a system update.
- **Then on real hardware,** once 550 gives it a network.
- **The record:**
  - the server build, the MIND Core release and the machine;
  - each operation's final state;
  - what was not checked.
- **Findings.** A difference between the server and the contract becomes either a new contract version (0001) or a report to the server's project.

## Acceptance criteria

- The record is in `docs/effector/` and cited by the profile for those configurations only.
- No compatibility is claimed for other server builds.

## Related

[750-EFF-0001](750-EFF-0001-protocol-contract.md), [750-EFF-0002](750-EFF-0002-test-server.md), [501](501-effector.md), [550](550-network-on-real-hardware.md), [docs/profile/evidence.md](../docs/profile/evidence.md).

## Progress — 2026-10-22: the first live session, against `hesere.uz`

The maintainer named the server, `https://hesere.uz/effector`.
- **The server.** It runs `security.mode=disabled`, said Effector's agent, so its agents carry no token. Contract 1.1 adds `token none` for that case.
- **The run.** `tests/qemu_smoke.py --suites effector-live`, x86_64 QEMU on the maintainer's laptop (LEG9), user networking.
  - **The agent:** `effector` from branch `claude/EFF-effector-agent`, agent ID `mind-qemu-x64-01`.
  - **The server:** `1.2.0.227`, commit `3c2f179e81f0`, which includes task 81's MIND boundary.
  - **The pins:** leaf `473d2d9f…bbe0` and SPKI `6df4cafe…a0bf` of the Let's Encrypt certificate valid until 2027-01-07.
- **Seen, for 121 s:**
  - TLS 1.3 to `37.140.216.52:443`, pinned;
  - no heartbeat or poll refused;
  - the server's polls said the agent waits for the owner's activation;
  - a shutdown heartbeat at the end.
- **Not seen yet:**
  - the endpoint as the server lists it;
  - activation;
  - any command;
  - ten minutes online;
  - aarch64 or hardware.
- **The pins change when Let's Encrypt renews the certificate,** if the key changes with it. A next pin is needed in `effector.conf` before then, or the agent refuses the server.

## Progress — 2026-10-22: the MacBook Pro, against `hesere.uz`

The first run on real hardware.

**The setup:**
- **The machine:** `fast-test` 85fefd4 (ABI 16, with 0119), `--slots --link-dev`, release b509a30.
- **Wi-Fi:** the BCM4331 on CoolNet2.
- **Through the link:** the owner's files were put under `data/link/effector/`. The owner then ran `setup.msh` at the keyboard: the files moved to `data/effector/`, `netpolicy add` was answered Y, and `data/mode.conf` was written (maintenance), then a restart.
- **The shell faulted** right after `netpolicy add` in the script. That is APP's, reported. The owner wrote the mode and restarted by hand.

**Seen:**
- **The services:**
  - `[INIT] MODE: MAINTENANCE`;
  - `init` started `effector_gw` (PID 37), then `effector` (PID 38);
  - the flow was granted, and the token was moved into `system/effector`;
  - agent `mind_4cd67c61c5c9496b` connected, pinned, to `hesere.uz` 227 at 10 s.
- **The server:** it listed `mind-macbook-pro` online. The owner activated it.
- **A `screenshot` command reached the agent** and was answered `denied`.
- **The connection:**
  - its ACK failed three times on transport errors;
  - the server ended both screenshot requests with `timeout` ("agent did not acknowledge");
  - in about 290 s there were 27 heartbeat, 4 poll and 1 ACK transport errors, and 32 new connections;
  - meanwhile `bcm_wifi` counted a fifth to two fifths of its frames lost, at 1 to 2 Mbit/s, 802.11b only (DRV, reported).
- **The gateway:** it held no `data/effector/policy`, so every action would answer `no-policy`.

**Not met:**
- a stable session on this machine;
- an ACK through;
- ten minutes online.

## Progress — 2026-10-23, the MacBook Pro on a stick from `fast-test` 0d73c076 (ABI 17)

- **The configuration:** the maintenance mode written with the disk; `effector.conf` with `token none` and the owner's policy (`screen on`, `log ring`, `log boot`, `collect log:/`, `collect data/media/`, `deliver data/inbox/`, `max-bytes 2097152`) put in place by the owner's script over the link; Wi-Fi BCM4331 at 11 Mbit/s CCK. The rewrite wiped `system/effector`, so the agent made a new ID, `mind_353abc18de2dbd0e`, activated by the owner as `mind-mbp2`.
- **The gateway's grants as `init` gave them:** SYSINFO LIFECYCLE LOG READER WRITER PRIVATE DISPLAY.
- **Telemetry on `hesere.uz`** (`GET /api/v1/monitoring`, 02:28 UTC): `Intel(R) Core(TM) i7-3615QM CPU @ 2.30GHz`, 8 CPUs, 5.8% busy, 8 160 358 400 bytes of memory, the boot, `ram` and `log` volumes, card 2 wireless with its MAC, 47 services with their PIDs, processor time and memory; the server counted the agent's traffic by `X-Effector-Agent-ID`.
- **A screenshot asked through the server's API** (operation `20261023022853.806949911_000330`): `completed` in 4 s, a 1280x800 PNG of 124 829 bytes, SHA-256 `e39c9096…`, which the downloaded artifact matches; it shows the machine's console.
- **Still seen:** about one transport error in 10–15 s on this Wi-Fi (DRV's 802.11b losses); requests and the ACK went through on reconnects.
