# 700-EFF-0011 — The owner's guide, the profile and the threat model

**Type:** documentation · **Owner:** `EFF` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** 0006–0008 (what the guide describes) · **Main task:** [700](700-effector-agent.md) · **Constitution:** MC-12.1, MC-12.3, MC-12.9, MC-10.7

## Problem

- **The owner** needs to know how to set up the agent, what the server can then do on the machine, and how to take that back.
- **The profile** must say what holds and on which configuration, and what does not (MC-12.3).

## Plan

- **`docs/effector/README.md` and `README_RU.md`:**
  - what the agent is;
  - how to enable it, and how to write the configuration and the policy with the shell;
  - what each policy line allows;
  - how to see the audit trail, and how to turn the agent off and revoke the token;
  - which Effector server builds it was checked with.
- **`docs/profile/`:**
  - **`threat-model.md`:** the Effector server as an adversary, and what the policy bounds;
  - **`bootstrap.md`:** what `init` gives each of the two services;
  - **`network.md`:** the flow, `tls` with a pinned certificate;
  - **`../network/airlock.md`:** the rows for `effector` and `effector_gw`;
  - **`README.md`:** the rows for MC-10.2, MC-10.7, MC-11.6 and Article 7.
  - Each claim names its suite and configuration.
- **`docs/tools/`:** the shell's `effector` command, when the tools track adds it.

## Acceptance criteria

- The guide exists in both languages.
- The profile's tables are updated in the same commit as the behaviour they describe. Nothing measured in QEMU is claimed for a physical machine.

## Related

[docs/profile/README.md](../docs/profile/README.md), [docs/profile/threat-model.md](../docs/profile/threat-model.md), [docs/profile/bootstrap.md](../docs/profile/bootstrap.md).
