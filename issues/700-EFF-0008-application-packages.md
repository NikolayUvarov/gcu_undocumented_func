# 700-EFF-0008 — Signed application packages: install, update and remove an application

**Type:** service feature · **Owner:** `EFF` track (open) · **Priority:** P1 · **Status:** open · **Blocked by:** [700-EFF-0005](700-EFF-0005-zip-reader.md), [700-EFF-0007](700-EFF-0007-gateway-service.md); [requests-UPD.md](requests-UPD.md) (signed application packages: format, key role, verification); [requests-KRN.md](requests-KRN.md) (a writable scoped `vfs` client; the loader's check of an installed application); the Effector server's package hash and non-shell entry point (its task 75, part B) · **Main task:** [700](700-effector-agent.md) · **Constitution:** MC-8.5, MC-9.2, MC-9.4, MC-9.8, MC-3.11, MC-6.6

## Problem

- **What the server can do.** Effector installs software by sending a package. On Windows and Linux the agent runs a script from it.
- **What MIND Core has.**
  - No package format.
  - Applications are not verified when they are loaded.
  - Nothing an agent may write to holds programs.
- **What must not happen.** A package from the network must not become running code on the strength of a TLS connection alone (MC-9.2, MC-9.8).

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

[351](351-self-update.md), [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [700-EFF-0005](700-EFF-0005-zip-reader.md), [docs/profile/README.md](../docs/profile/README.md) (applications not verified: "Not met").
