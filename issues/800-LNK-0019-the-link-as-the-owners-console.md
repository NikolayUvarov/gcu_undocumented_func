# 800-LNK-0019 — The link as the owner's console, in a mode the owner turns on: `link-console`

**Type:** service, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-10-29) · **Blocked by:** — (`KRN`'s 800-KRN-0155 and `APP`'s 800-APP-0108 are in `main`) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-3.11, MC-8.1, MC-12.3, MC-12.6

## Problem

On 2026-10-23 the MacBook Pro was rewritten with `--mode=maintenance`. The Effector agent's files then had to be put back:
- they were put into `data/link/effector/` over the link;
- `msh /data/link/effector/start.msh`, sent from `effector485`'s page, was refused: "the shell refused this line from the link";
- the script moves the files into `/data/effector` and restarts `effector_gw` and `effector`.

The shell refuses the link's client any script, any change of the network policy and any change of the machine's modes (800-APP-0102). So the step needs someone at the Mac's keyboard.

The maintainer's word, 2026-10-23: "effector and effector485 have to have equal rights in maintenance mode, or we need an additional mode line for the 485 debug link".

## Why a mode of its own, not the agent's rights

- **The agent's rights don't cover this step.** In the maintenance mode, the agent acts through `effector_gw`, within the owner's policy, with an audit line per request (docs/effector/README.md). It cannot put its own configuration in place: that is the owner's step at the console, which `start.msh` is.
- **What the step needs is the console's rights,** and the cable gets them only when the owner says so (MC-12.6, MC-3.11):
  - a word in `data/mode.conf`, written with the disk or turned on at the keyboard;
  - never by the cable itself.

## Plan

1. **`KRN`, `init`:**
   - `link-console` is a word of `data/mode.conf` beside `development` and `maintenance`;
   - it counts only with `dev` in `link.conf`; without it, it is logged and left out;
   - the boot line names it, `[INIT] MODE: MAINTENANCE AND LINK CONSOLE (data/mode.conf)`;
   - `init.wit`'s mode state gains it, under a new version.
2. **`APP`, the shell:** while the mode is on, the link's client is taken as the console's user:
   - a script by its file runs, with what its `requires:` asks;
   - what a program's client is asked runs at once, the network policy's changes included;
   - `mode` stays refused over the link, so the cable cannot turn its own mode on or off;
   - `mode link-console on|off` at the console.
3. **`LNK`, this task:**
   - `build_and_write.sh --mode=` takes `link-console`;
   - the `link` suite checks, with the mode, that `sh msh <a script in data/link>` runs and its output comes back; without it, the same line is refused;
   - `docs/link/` and `docs/write-disk.md`, English and Russian.
4. **Later, if asked:** the link as a second front end of `effector_gw`, under the owner's policy and its audit. That is the agent's rights over the cable, for when the agent's server is out of reach.

## Acceptance criteria

- [x] **QEMU:** (the `link` suite, x86, 4 CPUs, 2026-10-29)
  - the `link` suite with `dev` and `link-console`: `msh` of a script in `data/link/` runs and its lines are logged whole;
  - without the mode, the same line is refused, and `mode` is refused in both cases.
- [ ] **On the MacBook Pro:** `msh /data/link/effector/start.msh` from `effector485` puts the agent's files in place, and the agent connects.

Evidence for the configurations named only (MC-12.1). What the cable may do in the mode is the shell's and `init`'s to enforce. This task claims no more than they grant (MC-12.3).

## Progress

- **2026-10-23, `KRN`'s part is made** (800-KRN-0155, `fast-test` 889f5bae, landing in round 16):
  - `init` takes `link-console` in `data/mode.conf` when `link.conf` has `dev`, and leaves it out otherwise;
  - `init.wit` 1.8 adds `link-console()`.
- **2026-10-23, the script writes the word.** `scripts/build_and_write.sh --mode=` takes `link-console` beside `development` and `maintenance`, for example `--mode=maintenance,link-console`. It refuses the word without `--link-dev`, as `init` would leave it out. `docs/write-disk.md` and its Russian version say so.
- **2026-10-28, `APP`'s part is in `main`** (800-APP-0108): the shell takes the link's lines as the console's in the mode: a script by its file and the network policy's changes, never `mode`.
- **2026-10-29, the `link` suite's checks** (`tests/qemu_smoke.py`, on `main` 5a09c53d, x86, 4 CPUs, passed):
  - **with `dev` and `link-console` in `data/mode.conf`:**
    - `init` logs `MODE: LINK CONSOLE`;
    - a script put over the link, `sh msh /data/link/console.msh`, runs, and its output comes back;
    - `link` logs the whole line, and the shell logs `FOR THE LINK'S CONSOLE THE SHELL RAN MSH`;
    - `mode` is refused;
  - **with `dev` alone:** the same line is refused, and `mode` too.
- **Left:** the MacBook Pro's run.

## Related

[800-LNK-0015](../issues-done/800-LNK-0015-shell-commands-over-the-link.done), [800-LNK-0014](800-LNK-0014-build-and-write-in-the-repository.md) (`--mode=`), [750-KRN-0143](750-KRN-0143-init-follows-the-development-and-maintenance-modes.md), [750-EFF-0013](750-EFF-0013-development-and-maintenance-modes.md), `shell/src/clients.rs`, [docs/effector/README.md](../docs/effector/README.md).
