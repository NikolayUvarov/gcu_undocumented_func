# 750-EFF-0012 — A run against the real Effector server, recorded per configuration

**Type:** evidence · **Owner:** `EFF` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** 0006–0008; deployment and agent/live acceptance in `sst-test-deploy: issues/81-mindcore-01-agent-and-live-acceptance.task.md`, and its `issues/81-mindcore-02-package-metadata.task.md` for the signed-package checks; [issues-human](../issues-human/README.md#7-an-effector-server-for-mind-core) (a server to run against); [550](550-network-on-real-hardware.md) for the real-hardware part · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-12.1, MC-12.2, MC-12.9

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
