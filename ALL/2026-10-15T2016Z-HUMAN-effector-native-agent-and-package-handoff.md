# Effector asks MIND Core to deliver its native agent and package contract

**From:** HUMAN (the maintainer's explicit request, relayed by Codex working on Effector) · **To:** EFF, UPD, KRN; ALL for coordination · **Written:** 2026-10-15 20:16 UTC · **Answers:** — · **Concerns:** 750, 750-EFF-0003/0006/0007/0008/0012, 501 · **Answer wanted:** yes

The binding request is on `main` at
`bd7b5493d22f5d818c5c07b07ba4873dbaba4b60`:
[issues/requests-EFF.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/bd7b5493d22f5d818c5c07b07ba4873dbaba4b60/issues/requests-EFF.md),
linked from task 750 and the issue index. It is documentation only and needed
no build/gate under AGENTS.md section 4.

The maintainer explicitly wants MIND Core to implement its own native agent.
The Effector server project is not taking the EFF track or implementing the
MIND services. Please continue the existing tasks rather than duplicate them.

Effector's local canonical server change is
`be9d62c33712c8576210fdf8362a7409f49ad4ae`: isolated MIND programs and paths,
data-only delivery, unsupported native binary updates, and explicit refusal
of legacy MIND package deployment until a signed native contract is agreed.
Its server tests passed. No deployment or real MIND endpoint is claimed.

MIND `ASTRA` at `1aea9100` has the initial contract and HTTPS fixture
(`bc48775d`, `17e7005c`); those do not supply the native services.
Please coordinate UPD/KRN's existing requests, agree the versioned package,
signature, architecture, non-shell entries and exact archive binding, and
return a native build and configuration for joint acceptance. The detailed
deliverables and evidence matrix are in the binding request.

Please answer in `HUMAN/`, citing this message, with the responsible MIND
tasks/owner, contract location/version and first build available for testing.
Keep the outcome in the main-branch tasks/request. Provisioning credentials
stay in the owner's private channel; no secrets belong in this branch.
