# 800-KRN-0144 — In developer mode, `init` replaces a service with an image from `data/link/`, with no restart

**Type:** `init`, IDL (`init.wit`, a new minor version) · **Owner:** kernel session · **Priority:** P2 · **Status:** open (2026-10-21) · **Blocked by:** — · **Main task:** [800](800-mind-link.md) (the link track's) · **Constitution:** MC-3.4, MC-12.1, MC-12.3

From the link track's request of 2026-10-21, for the driver track's work on the MacBook Pro (`internal-dev-messages`, `KRN/2026-10-21T1233Z-LNK-…`). The maintainer asked for a debug mode in which the agents test on the Mac with no person at it.

## Problem

- **What a driver change costs today.** DRV changes one service, `bcm_wifi`, many times a day (550-DRV-0029).
  - Each change needs a new image on the USB disk, written on another machine, and a boot from it.
  - Since 2026-10-21 the Mac comes back from a restart by itself in about 20 s. A restart still loses the state being tested, and the new image still has to reach the disk.
- **What the link can do.** In developer mode (`dev` in `link.conf`, [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md)), the agent at the other end puts files into `data/link/` and asks `init` for a restart of the machine. It cannot ask `init` to start a service from a file.
- **What stays.** The boot chain checks every boot image against the signed manifest. Nothing here changes that: what is replaced runs unverified only until the next boot.

## Plan

1. **A request on `init`'s client:** `replace(unit, path)`, `init.wit` 1.5 or the next minor version, shared with [750-KRN-0143](750-KRN-0143-init-follows-the-development-and-maintenance-modes.md) if both land together.
   - **Who may ask.** Only `link`'s client in developer mode, by a badge of its own (`BADGE_REPLACE`), minted only when `link.conf` holds `dev`. Others get `denied`.
   - **What `init` does:**
     - stops the unit as `stop` does;
     - reads the ELF at `path`, which must name a file inside `data/link/`, through a read-only client confined there;
     - starts the unit from that image in place of its boot image, with the grants of its plan, as a restart keeps them.
   - Units that may not be replaced: `init`, `vfs_server`, `logd` and `link` itself (the request would cut its own path). Others answer `denied`.
2. **Said, never hidden.**
   - The log line is `[INIT] <unit> REPLACED FROM data/link/<path>: SHA-256 <digest>, NOT VERIFIED`.
   - `list` marks the unit as replaced, so `svc` and `sysinfo`'s readers show it.
3. **Back to the verified image.**
   - A restart of the unit after a fault comes from the replaced image while the machine runs.
   - `replace(unit, none)` returns it to its boot image.
   - A restart of the machine always does.
4. **Without `dev`, nothing changes:** no badge is minted, and `replace` is refused.
5. **The profile.** `docs/profile/bootstrap.md` says that in developer mode a replaced service runs unverified until the next boot, and that evidence from such a run holds for that configuration only (MC-12.1, MC-12.3).
6. **The check (QEMU, x86).** A suite replaces a stand-in service with an image put into `data/link/` and checks:
   - the log line;
   - the replaced unit's grants, the same as before;
   - `replace(unit, none)` going back to the boot image;
   - a refusal without `dev`.

   The link track's `mindlink replace` drives it from the other end once it exists.

## Acceptance criteria

- [ ] In QEMU, with `dev`, a service replaced from `data/link/` runs with its plan's grants, the log says it is not verified, and `replace(unit, none)` brings the boot image back.
- [ ] Without `dev`, the request is refused and nothing is minted.
- [ ] `docs/profile/bootstrap.md` states what this mode runs unverified.

## Related

[800-KRN-0139](800-KRN-0139-the-links-developer-mode.md), [800-LNK-0012](../issues-done/800-LNK-0012-the-developer-mode.done), [750-KRN-0143](750-KRN-0143-init-follows-the-development-and-maintenance-modes.md), [000-KRN-0124](000-KRN-0124-switching-between-linux-and-mind-core.md) (`BootNext`), 550-DRV-0029.
