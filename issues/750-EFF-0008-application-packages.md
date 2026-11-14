# 750-EFF-0008 — Signed application packages: install, update and remove an application

**Type:** service feature · **Owner:** `EFF` track · **Priority:** P1 · **Status:** in progress (2026-11-14) · **Blocked by:** [750-EFF-0005](../issues-done/750-EFF-0005-zip-reader.done), [750-EFF-0007](750-EFF-0007-gateway-service.md); [750-UPD-0019](750-UPD-0019-signed-application-packages.md) (signed application packages: the contract, key role, verification); [requests-KRN.md](requests-KRN.md) (a writable scoped `vfs` client; the loader's check of an installed application); the open Effector package metadata/non-shell entry-point requirement in `sst-test-deploy: issues/81-mindcore-02-package-metadata.task.md` · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-8.5, MC-9.2, MC-9.4, MC-9.8, MC-3.11, MC-6.6

## Problem

- **What the server can do.** Effector installs software by sending a package. On Windows and Linux the agent runs a script from it.
- **What MIND Core has.**
  - No package format.
  - Applications are not verified when they are loaded.
  - Nothing an agent may write to holds programs.
- **What must not happen.** A package from the network must not become running code on the strength of a TLS connection alone (MC-9.2, MC-9.8).
- **External requirement still open.** The server must bind a command to exact package bytes with size/SHA-256 and select explicit MIND architecture/install/uninstall entries naming a signed payload, without a Windows/Linux or shell-script fallback. External task `81-mindcore-02-package-metadata.task.md` records that work and its joint acceptance. Task 81's local platform/configuration/path fix does not provide this package contract and has not been deployed; task 81-01's basic agent/live acceptance does not close it either.

## Plan

- **The package.** An Effector deployment ZIP (unencrypted) whose `manifest.json` has an install and an uninstall entry for `os: "mindcore"` and the architecture. Each entry names a MIND Core package in the archive:
  - its manifest: name, version, files with SHA-256, the requests (`.mind_request`) of each program;
  - a signature by a key whose role is "application package" (requests-UPD; 351-UPD-0009).
- **Install.**
  - Check the signature against the keys the owner's policy trusts.
  - Check every file's hash.
  - Refuse a version lower than the installed one unless the policy allows it (MC-9.4).
  - Write to `data/apps/<name>/<version>/`.
  - Switch `data/apps/<name>/current` atomically.
  - Keep the previous version.
- **Update an installed application.** Install the newer version (reinstall); the old one stays until the next update.
- **Uninstall.**
  - Remove the current version's switch, then the files.
  - Running instances are stopped first if the policy allows, otherwise the request is `denied`.
- **Never touched:** applications on the boot volume, boot services and boot files. They change only through system update (0009).
- **Reporting.** Each step goes to the deployment result route and the ACK, as the contract says. The package's manifest requests and never grants: the shell's lending still decides at launch (MC-3.11).

## Acceptance criteria

- **In the `effector` suite, against the 0002 server:**
  - a signed package installs and its program runs from `data/apps/`;
  - a newer version replaces it and the previous one stays;
  - uninstall removes it;
  - an unsigned, altered, encrypted or older package is refused, and nothing is written.
- The loader refuses an installed program whose files changed after installation (with the KRN part).

## Related

[351](351-self-update.md), [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [750-EFF-0005](../issues-done/750-EFF-0005-zip-reader.done), [docs/profile/README.md](../docs/profile/README.md) (applications not verified: "Not met").

## Progress (2026-10-30)

- **Points 1 and 2 of the package contract are drafted** in [docs/effector/packages-draft.md](../docs/effector/packages-draft.md), contract 1.5 draft 1, for the Effector server project's review. They are native EFF's since 2026-10-24.
  - The command: `deploy` with `new_hostport` `mind-package-1:<operation>:<package_id>:<size>:<sha256>`.
  - The download: only `/api/deployment/download/{id}`, bound to that size and SHA-256.
  - The result: `DeploymentResult` with `script` `mind-package-1` and one `stdout` line, then the ACK, both with the same outcome and refusal code.
- Nothing of it is built. The install waits for the server's confirmation and for KRN's 750-KRN-0169.

## Progress (2026-11-14): built to contract 1.5 draft 2; host tests pass, the QEMU check is not run yet

**What the server needs is on its side:** the Effector project's `mindcore_packages.go` sends `mind-package-1` behind `mindcore_packages_enabled`, which stays off on real servers until this lands.

**The agent** (`effector/src/main.rs`, `wire.rs`):
- `deploy` with `mind-package-1:<operation>:<package_id>:<size>:<sha256>` in `new_hostport` (`wire::package`). Any other form, the host form among them, is `unsupported`. The budget is a fixed 600 s.
- The archive comes from `/api/deployment/download/{id}` straight into the gateway: `package-begin`, `write`, `package-commit`.
- Then `POST /api/deployment/result`, the server's `DeploymentResult` with `script` `mind-package-1` and one `stdout` line:
  - `installed NAME VERSION ARCH FILES MANIFEST_SHA256`;
  - `removed NAME VERSION`;
  - `refused CODE: …`.
- Then the ACK, with the same outcome: `completed` with that line, or `failed` with the code.
- `session::Failure` gains `signature`, `older`, `format`, `arch`, `system` and `space`.

**The gateway** (`effector_gw/src/gateway.rs`, `package.rs`, `main.rs`; `effector.wit` 1.7):
- **At `package-begin`:** `packages on` and `max-bytes` (`Ask::Packages`), audited before any byte. An archive over 8 MiB is `unsupported`, as the gateway reads it whole in its 16 MiB heap.
- **The archive** goes to `data/apps/.incoming.zip` in the apps scope (`SLOT_APPS`). At the commit its size and SHA-256 are checked.
- **`package::run`:**
  - `manifest.json`'s `mindcore` entry for this architecture, by the contract's rules;
  - MANIFEST and MANIFEST.SIG from that directory;
  - `apppkg::verify` under the policy's `package-key` lines for the name, this architecture and the running release. Without a `package-downgrade` line, a lower version or the same version with other bytes is `older`;
  - each file against its MANIFEST line before it is written.
- **Where an install writes:** `<name>/.new-<version>/`, which then becomes `<name>/<version>/`. After that, `current` is written as `current.new` and renamed. The previous version stays.
- **A deinstall** checks the package's signature, then removes `current`, then the application's directory.
- **Each outcome is audited.**

**Host tests:**
- `tests/effector_package_host.rs`, 6 tests: the entry's rules; install, update and the previous version kept; each refusal leaving the installed version and nothing half-written, a full disk among them; the removal;
- the gateway's transfer, its digest and its audit (`effector_gateway_host.rs`);
- the policy's `Ask::Packages`;
- the command's form and the result's fields (`effector_wire_host.rs`).

**The QEMU check, written but not run:** `effector_packages_check` in the effector suite, x86 and aarch64, with `memtest` signed by the public test key:
- install, then run from `data/apps/`;
- version 2 in its place, 1 kept;
- refusals for another key, an older version and a changed program (`signature`, `older`, `digest`), each changing nothing;
- the removal;
- each command's DeploymentResult and ACK.

**Not done:**
- **The `busy` check of a running application** at `reinstall` and `deinstall`.
- **Pruning versions older than the previous one.**
- **An atomic `current`:** the volume has no replacing rename. So `current` is removed, then `current.new` is renamed onto it. A power loss between the two leaves the application without `current` until the next install, never a mix of versions.
- **Archives over 8 MiB.**
- **A run against the Effector server itself,** once its flag is on (750-EFF-0012).

