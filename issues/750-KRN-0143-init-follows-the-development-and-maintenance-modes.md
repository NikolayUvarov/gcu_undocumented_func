# 750-KRN-0143 — `init` follows the development and maintenance modes: Effector's services only in a mode

**Type:** `init`, IDL (`init.wit` 1.5), ABI (`SLOT_MODE`, ABI 14) · **Owner:** kernel session · **Priority:** P1 · **Status:** the mode in `main` (3bdd026, ABI 14, 2026-10-21); the services' part and the shell's command left · **Blocked by:** the two services ([750-EFF-0006](750-EFF-0006-session-service.md), [750-EFF-0007](750-EFF-0007-gateway-service.md)) and their boot images ([750-KRN-0119](750-KRN-0119-boot-images-and-grants-for-effector.md)) for the services' part; nothing for the mode itself · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-3.8, MC-8.1, MC-10.2, MC-12.3, MC-12.6

From the agent track's request of 2026-10-21 (`internal-dev-messages`, `ALL/2026-10-21T1135Z-EFF-…`). The design is 750-EFF-0013's, on the agent track's branch.

## Problem

- **The maintainer's decision (2026-10-21).** The Effector agent manages every aspect of the OS during development, so it needs high privileges.
  - It runs **only in the development mode or the maintenance mode**, and in a mode it is a service that runs all the time.
  - A mode is turned on explicitly, by the operator, or by the Mind when it needs help from outside.
  - A mode is set by a word and by a command, and it holds across any number of reboots.
- **What was planned.** [750-KRN-0119](750-KRN-0119-boot-images-and-grants-for-effector.md) item 2 started both services when the owner enabled them in `data/services.txt` ([173-KRN-0035](173-KRN-0035-init-reads-the-service-configuration.md)), with no mode.
- **What `init` does now.** It knows no mode. The only word it follows is the link's `dev` (800-KRN-0139), which the bootloader reads from `link.conf`.

## Plan

1. **The mode's file: `data/mode.conf`** on the boot volume, as 750-EFF-0013 proposes.
   - It holds `development` or `maintenance`, or neither (the normal mode). Other words are ignored and logged.
   - **`init` reads it** after `vfs_server` starts, through a client of its own confined to that one file, before it starts the Effector services.
   - **The bootloader does not read it.** The mode decides only which services `init` starts, so no boot information (ABI) changes for it.
   - The maintainer's disk-writing script may write the word when the disk is written, as `--link-dev` writes `link.conf`'s `dev`.
2. **At boot.**
   - **With a word,** `init` starts `effector` and `effector_gw` and restarts them as it restarts other boot services.
   - **Without one,** it starts neither and mints no Effector grant.
   - The boot line names the mode: `[INIT] MODE: DEVELOPMENT (data/mode.conf)`.
3. **At run time: `init.wit` 1.5** (a new minor version; 1.4's functions keep their meaning).
   - `mode: func() -> result<mode-state, error>`: the mode, who set it and when.
   - `set-mode: func(mode: mode, on: bool, reason: string<120>) -> result<_, error>`.
     - **`init` writes the file itself,** so the file and the running services cannot disagree, and no other holder needs to write it.
     - Turning a mode on starts both services at once; turning it off stops both, which ends the session. No reboot.
   - **Who may call it:**
     - an `init` client badged `BADGE_MODE`, the shell's alone, in a slot of its own: `SLOT_MODE` (36), so ABI 14 moves `SLOT_DYNAMIC` to 37. Not `SLOT_INIT`: the shell lends that client to programs as `SLOT_LIFECYCLE`. The shell asks the operator at the local console before it turns a mode on (`APP`, the `mode` command);
     - an `init` client badged `BADGE_MIND`, which may only turn the maintenance mode on, with a reason. No component holds it yet; the Mind gets it when it exists (MC-8.1, MC-12.6);
     - any other caller is refused (`denied`).
4. **Audit.** Each change is one line in the system log: who (the badge and the caller's PID), which mode, on or off, the reason (MC-10.2). The agent track's audit record reads it from there or from `mode`.
5. **750-KRN-0119 follows the mode** instead of `data/services.txt`. Its item 2 now points here.
6. **The check, in QEMU (x86 and aarch64).** Until the services exist, a test image starts a stand-in program for each, so the mode can be checked before 750-EFF-0006 and 0007 land.

## Acceptance criteria

- [ ] Without a mode word, neither service runs, and no process holds an Effector grant (`stat caps`).
- [ ] With `development` written when the disk was written, both run from boot and are started again after they are killed; after three reboots the mode still holds.
- [ ] `set-mode` through `BADGE_MODE` starts and stops both with no reboot, and writes the file.
- [ ] `BADGE_MIND` turns the maintenance mode on with a reason, and cannot turn a mode off or set the development mode; a holder without either badge is refused.
- [ ] Every change is logged with its badge, PID and reason.
- [ ] A safe start with the maintenance mode on starts both services.

## Progress (2026-10-21)

**The mode itself is in `main`** (landing round 5, gate `land-4f67455f`, `main` 3bdd026); the Effector services' part waits for their boot images (750-KRN-0119).
- **ABI 14:** `SLOT_MODE` (36), the shell's alone and no launch slot; `SLOT_DYNAMIC` 37. `BADGE_MODE` (3) and `BADGE_MIND` (4) in `mind::process`. `tests/runtime.rs` checks the slots.
- **`init.wit` 1.5:** `enum mode`, `record mode-state`, `mode()` and `set-mode(which, on, reason)`; `libmind/src/idl/init.rs` regenerated.
- **`init`:**
  - it reads `data/mode.conf` once the boot volume is mounted, through a client with the user's rights that it drops at once;
  - it logs `[INIT] MODE: …` and each word it leaves out;
  - `set-mode` writes the file whole, then logs `[INIT] MODE <which> ON|OFF BY operator|mind (PID, BADGE): <reason>`; a refusal logs the caller's PID and badge;
  - the Effector services follow the modes once they exist; this build says it has none.
- **`check services`:** "the modes" reads them; "a mode change refused" asks with its lifecycle client and must be denied (both in the bench suite's `CHECKS_PASS`).
- **The `modes` QEMU suite**, in "x86: services, storage, audio" and "aarch64: files, network and TLS" (both passed locally): the development mode and an unknown word from `data/mode.conf`, the shell's `SLOT_MODE` badged 3 and no other client of `init` so badged, `check services`' two rows, the refusal logged; a second boot with no file in the normal mode.
- **Left:**
  - the services' part, with 750-KRN-0119;
  - `set-mode` through `BADGE_MODE`, once the shell's `mode` command (APP) calls it;
  - `BADGE_MIND`'s holder, the Mind;
  - the three-reboot criterion.

## Progress (2026-10-22)

**The services' part** is made with 750-KRN-0119 (ABI 16):
- both Effector services start once a mode is read, and stop when none is on;
- a client may not start them without a mode;
- the `modes` suite checks this both ways (x86).

**Left:**
- `set-mode` through `BADGE_MODE`, once the shell's `mode` command calls it;
- `BADGE_MIND`'s holder;
- the three-reboot criterion;
- the safe start's maintenance case.

## Settled

- **A safe start** (000-KRN-0076) starts both services when the maintenance mode is on: help from outside is what a machine that failed to boot needs (the maintainer, 2026-10-21, through EFF). Whether a safe start keeps the development mode was not asked.

## Related

[750](750-effector-agent.md), 750-EFF-0013 (the design, on `claude/EFF-effector-agent`), [750-KRN-0119](750-KRN-0119-boot-images-and-grants-for-effector.md), [750-APP-0093](750-APP-0093-the-shells-effector-command.md) (the `mode` command instead), [173-KRN-0035](173-KRN-0035-init-reads-the-service-configuration.md), [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md) (the link's `dev` word, the same pattern).
