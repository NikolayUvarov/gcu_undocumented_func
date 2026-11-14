# 801-LNK-0046 — Each machine's settings kept by `build_and_write.sh`, and put on any new disk

**Type:** tools (`scripts/build_and_write.sh`) · **Owner:** `LNK` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-4, MC-12.3

## Problem

The maintainer (2026-11-14): «Каждая перезапись должна сохранять локально настройки». The MacBook Pro's USB disk is to be replaced by a stick for the testing period, so the settings must reach a new disk too.

`--keep-settings`, `--keep-wifi` and `--keep-effector` save into one place, `~/.config/mind/settings` and its kin. That place holds whatever disk was read last:
- a write that skips the old disk takes another machine's files;
- in October the DEXP got the MacBook Pro's agent this way.

## Plan

1. **Kept per machine:** `~/.config/mind/machines/<id>/`, where `<id>` is the agent's ID in `system/effector/`, or the device key's public fingerprint when there is no agent. Never a secret in the name.
2. **Before a write:** the old disk is read, and its machine's directory is updated.
3. **After the write:** the settings of the machine named by `--machine <id>`, or by the old disk just read, are put on the new disk. A different device than the old one is allowed.
4. **No choice by default:** a write with neither an old disk nor `--machine` is refused. The script never falls back to "the last one".
5. **`--list-machines`** names the kept machines, by ID, host name and date, and no secret.

## Acceptance criteria

- **Tests on temporary directories:**
  - two machines kept side by side;
  - a write onto a new device from one machine's directory;
  - a write with no machine refused.
- **On the bench:** the MacBook Pro's settings put on a new stick, its agent back as the same endpoint, its Wi-Fi joined.

## Related

[801](801-the-bench-stays-manageable.md), [800-LNK-0023](800-LNK-0023-keep-the-effector-agent-across-a-write.md), [800-LNK-0024](800-LNK-0024-keep-the-owners-settings-across-a-write.md).
