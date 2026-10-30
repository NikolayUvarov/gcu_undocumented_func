# 750-UPD-0020 — The Effector gateway's client of `updater`

**Type:** service interface · **Owner:** `UPD` track · **Priority:** P2 · **Status:** in progress (2026-10-30: the badge and its gate made with 751-KRN-0185) · **Blocked by:** — (`init`'s part is [751-KRN-0185](751-KRN-0185-the-remote-update-permission-in-init.md)) · **Main task:** [750](750-effector-agent.md) · **Roadmap:** stage V, track C · **Constitution:** MC-7.5, MC-8.5, MC-9.3, MC-9.4

From the request "A client of `updater` for the Effector gateway (750)" in requests-UPD.md (recorded by the Effector agent track, 2026-10-09).

## Problem

- **What the server sends.** The Effector server asks an agent to update itself (`update_agent`).
- **What that can mean on MIND Core.** Only a system update through `updater` (351): the agent is part of the signed image (750-EFF-0009).
- **What the gateway needs.** `effector_gw` needs a client of `updater` that can ask for that. It must not be able to change, by this or any other path, what the device trusts or which channel it follows (MC-7.5, MC-8.5).

## Plan

- **The badge.** `idl/update.wit` gets a client badge for the gateway (`BADGE_UPDATE_GATEWAY`; the number is `KRN`'s, in `common/abi.rs`).
- **What the badge may do:**
  - `check`, `fetch` and `status`;
  - `apply` and `rollback` only if `init`'s arm for the gateway allows a restart.
- **What it may never change:** the channel, the trusted keys, the minimum version. Those stay with `update.txt` and the shell's `update` command.
- **`status`** gives what the gateway reports in its heartbeat:
  - the versions of the running, staged, trial and last-known-good releases;
  - the state;
  - the last error.

  If it needs fields `update.wit` 1.0 lacks, the interface gets a new minor version.
- **`init`'s part is a kernel task:** handing the gateway this client, and the arm. It is recorded in requests-KRN.md with the badge.

## Progress (2026-10-30)

- **The badge and its arm** came with [751-KRN-0185](751-KRN-0185-the-remote-update-permission-in-init.md):
  - `mind::process::BADGE_UPDATE_GATEWAY` (1);
  - `init` gives `effector_gw` the updater's client with it in `SLOT_UPDATE`, in a mode;
  - the arm is the owner's `remote-update` in `data/mode.conf`, not a separate one: from that badge `fetch`, `apply` and `rollback` run only while `init`'s `mode()` says it counts, asked at each (`update.wit` 1.2).
- **`check`, `status` and `metadata` stay open to it.** `update.wit` has no call that changes the channel, the keys or the minimum: those stay with `update.txt`.
- **Left:**
  - the `update` suite's run of a gateway-badged client (staged, then refused once the word is off);
  - the install of a developer's release the gateway receives (751-EFF-0023's `install`), with EFF.

## Landed (2026-10-30)

The gateway's client is in `main` since landing round 50 (8e2b16e0, 2026-10-30). The release the gateway hands over is [751-UPD-0032](751-UPD-0032-install-a-developers-release-from-the-gateway.md)'s `install` (`update.wit` 1.3), on the kernel session's branch with its QEMU suite on x86; this task's criteria follow from it.

## Acceptance criteria

- In the `update` suite, a client with the gateway's badge checks, fetches and stages a test release.
- It is refused when it tries to change the channel.
- Without `apply` in its arm, it cannot activate the release.

## Related

- [750-EFF-0009](750-EFF-0009-system-update.md), [750-UPD-0019](750-UPD-0019-signed-application-packages.md).
- [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done).
