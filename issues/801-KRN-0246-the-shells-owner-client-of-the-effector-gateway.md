# 801-KRN-0246 — The shell's owner client of the Effector gateway (`SLOT_EFFECTOR`, ABI 33)

**Type:** kernel (`common/abi.rs`, `init`) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S7) · **Roadmap:** stage V · **Constitution:** MC-3.8, MC-3.11, MC-12.4

## Problem

APP's request in `requests-KRN.md` (2026-11-14), for [801-APP-0140](801-APP-0140-effector-policy-in-the-shell.md) and 750-APP-0093:
- **No task holds the owner's badge.** `effector_gw` reads the owner's policy only when it starts, and on the owner's `reload-policy` (`idl/effector.wit`). Only a client badged `mind::effector::BADGE_OWNER` (2) may call that, and no task holds one.
- **So a policy change waits for the gateway's next start.** `effector policy allow update apply` (801-APP-0140) takes effect only then.
- **The shell's `effector status` and `audit` have nothing to ask.**

## Plan

- **`common/abi.rs`:**
  - `SLOT_EFFECTOR` 45 is the shell's alone and is not a launch slot;
  - `SLOT_DYNAMIC` moves to 46, `CAP_SLOTS` to 110, ABI to 33;
  - the handoff stays 35, which is above every ABI.
- **`init` gives the shell, at each of its starts, `effector_gw`'s client badged `BADGE_OWNER`.**
  - **Why one grant is enough:** `init` keeps the gateway's endpoint across its starts, so this one client reaches each start of the gateway.
  - **Without a mode:** no task serves the endpoint, so a call answers `ERR_PEER`.
- **Not as asked:** the request asked for no client while no mode is on and a new one at each start of the gateway. The kept endpoint gives the same result without lending anything into a running shell:
  - the owner's calls reach the gateway whenever it runs;
  - without a mode they are told it does not run.

  The same holds for the camera's prepared gateway (`SLOT_CAMERA`).
- **The badge allows no action:** `status`, `reload-policy` and `audit` only (`effector.wit`).
- **The shell never lends the client on.**

## Acceptance criteria

1. **The host test** names `SLOT_EFFECTOR` 45, `SLOT_DYNAMIC` 46 and ABI 33, and checks that a launcher may not fill the slot.
2. **The `modes` suite, x86 and aarch64:**
   - in the development mode, the shell holds the gateway's client with badge 2 in slot 45, and no other process but `init` and the gateway holds that badge of its endpoint;
   - without a mode, the shell holds it in slot 45 too, on an endpoint no task serves.
3. **The documents:**
   - `docs/api` names the slot and ABI 33;
   - the profile names it with the shell's clients, and gives the fixed slots 1–45.

## Related

[801](801-the-bench-stays-manageable.md), [801-APP-0140](801-APP-0140-effector-policy-in-the-shell.md), [750-KRN-0143](750-KRN-0143-init-follows-the-development-and-maintenance-modes.md) (`SLOT_MODE`), [308-KRN-0205](../issues-done/308-KRN-0205-the-boot-waits-for-keystores-passphrase.done) (`SLOT_PASSPHRASE`, ABI 31).
