# 501 — Tests on real hardware, driven through the effector

**Type:** main task · **Owner:** `ASR` track (open) · **Priority:** P2 · **Status:** open (proposed) · **Blocked by:** [700](700-effector-agent.md) (the effector agent, phase 2); [550](550-network-on-real-hardware.md) (the network on the MacBook Pro); from `NET` ([requests-NET.md](requests-NET.md), recorded for 351): TLS for a service other than the shell, and a persistent device key · **Roadmap:** Assurance "evidence tied to configuration" · **Constitution:** MC-12.1, MC-12.2, MC-12.9, MC-2.3, MC-10.2, MC-11.9

Asked by the maintainer (2026-10-08): tests should run on real hardware without the maintainer typing them. Opened by the kernel session; the `ASR` track is open.

**Revised at the maintainer's request (2026-10-09):** the server is the maintainer's Effector fleet server, and the effector on the target is the agent of main task [700](700-effector-agent.md) (track `EFF`). This task keeps the test runs: the bundle, the test account, and the record of each run per configuration. The connection, authentication, logs and updates are 700's.

## Problem

Every check on a real machine today is typed by the maintainer and read from photographs of the screen ([211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md)). The QEMU suites (`tests/qemu_smoke.py`) drive a guest through its serial line and QMP, which a laptop does not have. Evidence for a physical configuration is therefore rare, slow and made by hand. MC-12.1 asks for evidence per configuration, and QEMU's results do not carry over to a machine (MC-12.9).

## Design (proposed; the first bullets now describe 700's agent)

- **The effector** is a service on the target. It connects out over TLS (`tls`) to the one server named in its configuration, and checks the server's pinned key. The target needs no listening port.
- **It authenticates with the device key** (`keystore`), so the key must persist across boots (the `NET` request). The server keeps a list of the devices it accepts.
- **It receives a test bundle:** programs, `msh` scripts and their expected outputs, under a bundle ID.
- **It runs them in a test account:** a session with only the grants the bundle declares (as an `msh` script's `requires:` does), within a memory and time budget. A test gets none of the effector's own capabilities (TLS, the device key) and cannot widen its grants.
- **It streams back** the system log (from `logd`, under a read grant: MC-10.2), screenshots (as the shell's `screenshot` takes them, issue 086), and each test's output, exit status and time.
- **A new image only through self-update** ([351](351-self-update.md)). The server publishes a signed test release; the effector asks `updater` to stage and apply it, and the trial boot with last-known-good protects the machine. The effector never writes disks or boot files itself. Which key may sign a test release is a key-role question for 351-UPD-0009.
- **The server** is a small host program (in `scripts/` or `tools/`, proposed). It reuses `tests/qemu_smoke.py`'s checks where it can, so the same suite runs in QEMU and on hardware.
- **Evidence.** Each run is recorded with its configuration: the machine, the firmware and its settings, the image's manifest hash (350), the bundle and any seed. The profile cites a run for that configuration only. A passing run is evidence of what was run, not a proof (MC-12.2).
- **Its own risk.** The effector lets the server run code on the device, within the test account. It is off by default, turned on by the machine's owner, and [docs/profile/threat-model.md](../docs/profile/threat-model.md) names it when it lands.

## Plan: tasks

| Task | Track | What |
|---|---|---|
| 501-ASR-0007 | `ASR` (open) | **Withdrawn (2026-10-09), not written:** the protocol and the server are the Effector server's, fixed for MIND Core in [700-EFF-0001](700-EFF-0001-protocol-contract.md) and served in tests by [700-EFF-0002](700-EFF-0002-test-server.md) |
| 501-ASR-0008 | `ASR` (open) | **Withdrawn (2026-10-09), not written:** the client service is 700's `effector` and `effector_gw` ([700-EFF-0006](700-EFF-0006-session-service.md), [700-EFF-0007](700-EFF-0007-gateway-service.md)) |
| 501-ASR-0009 | `ASR` (open) | A test bundle as a signed Effector package ([700-EFF-0008](700-EFF-0008-application-packages.md)), run by `effector_gw` in the test account; a host driver that queues bundles through the Effector server's operator API and records each run with its configuration; numbered here, written when 700's phase 3 lands |
| `init`'s grants (not numbered yet) | `KRN` | The connection's grants are now 700's request ([requests-KRN.md](requests-KRN.md)); what remains here is screen capture and the test account's spawn rights for `effector_gw`, requested by the kernel track's numbering when 501-ASR-0009 needs them |

501-ASR-0007 and 501-ASR-0008 were numbered on 2026-10-08 and withdrawn on 2026-10-09 without files; their numbers are not reused. 501-ASR-0009 is written when work on it starts.

## Acceptance criteria

- In QEMU: the server sends a bundle, the effector runs it in the test account, and the results, the log and a screenshot come back. A test that uses a capability its bundle did not declare is refused. A broken connection is resumed or reported.
- On the MacBook Pro, over the network of 550: one of the QEMU suites' checks runs unattended, and its record names the machine's configuration.
- [docs/assurance/README.md](../docs/assurance/README.md) and the profile say what the effector's runs cover, and that they are not proofs.

## Related

[700](700-effector-agent.md), [500](500-fuzzing-abi-and-idl.md), [350](350-signed-boot-images.md), [351](351-self-update.md), [550](550-network-on-real-hardware.md), [650](650-building-on-the-target.md) (builds through a server), [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md), `tests/qemu_smoke.py`, [docs/profile/evidence.md](../docs/profile/evidence.md).
