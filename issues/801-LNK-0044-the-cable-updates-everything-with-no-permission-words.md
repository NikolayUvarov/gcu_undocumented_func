# 801-LNK-0044 — The cable updates everything with no permission words

**Type:** link (`link`, `tools/mindlink`, `effector485`, `scripts/link_update.py`) · **Owner:** `LNK` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — (the grants in `init` go to `KRN` as a request) · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-9.3, MC-9.4, MC-12.3

## Problem

The maintainer (2026-11-14): «effector485 должен уметь обновлять весь код, любой абсолютно без всяких разрешений». Today the cable's reach depends on words on the disk:
- `link.conf` must exist, with `run` and `dev`, for `replace`, `reboot`, the shell's lines and keys;
- `update.txt` must name `data/link/rel` for a slot update;
- without `link.conf`, `link` does not start at all.

## Plan

1. **`link` starts whenever a supported converter is attached.** `link.conf` only sets its speed and address.
2. **Over the cable, without any word:**
   - a slot update of every component, from the loader to the applications: the updater takes `data/link/rel` as a source whatever `update.txt` says;
   - `replace` of a service;
   - `reboot`, the log, files under `data/link/`, keys and the shell's lines.
3. **Releases stay signed** (MC-9). An unsigned one is refused, as now.
4. **`docs/profile/threat-model.md` and `docs/link/`:** the cable is physical access to the machine.

## Acceptance criteria

- **QEMU:** a disk with no `link.conf` and no `update.txt` takes a slot update and a `reboot` over the link's serial line.
- **On the bench:** the same on one machine.

## Related

[801](801-the-bench-stays-manageable.md), [800](800-mind-link.md), [351-LNK-0013](351-LNK-0013-staging-a-release-through-the-link.md).
