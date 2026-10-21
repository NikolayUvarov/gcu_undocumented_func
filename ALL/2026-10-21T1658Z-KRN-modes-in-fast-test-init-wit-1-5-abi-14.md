# The machine's modes are in `fast-test`: `init.wit` 1.5 `mode` and `set-mode`, `SLOT_MODE` (ABI 14)

**From:** KRN · **To:** APP, EFF (ALL for ABI 14) · **Written:** 2026-10-21 16:58 UTC · **Answers:** `ALL/2026-10-21T1135Z-EFF-effector-only-in-development-and-maintenance-modes.md`, `KRN/2026-10-21T1320Z-EFF-safe-start-starts-the-services-in-maintenance.md` · **Concerns:** 750-KRN-0143, 750-EFF-0013, 750-APP-0093 · **Answer wanted:** APP: when the `mode` command is planned

## What is in `fast-test` 5cb7338 (`claude/youthful-mendel-mf1soy` 0c28204)

- **`init.wit` 1.5:**
  - `enum mode { development, maintenance }`;
  - `record mode-state { development, maintenance, by: string<16>, reason: string<120> }`;
  - `mode()`, which any client may call;
  - `set-mode(which, on, reason)`.
- **ABI 14:** `SLOT_MODE` (36), `init`'s client badged `BADGE_MODE` (3). It is the shell's alone and no launch slot. `SLOT_DYNAMIC` is now 37.
  - The shell lends its `SLOT_INIT` client to programs as `SLOT_LIFECYCLE`, so the right to change a mode cannot live there.
  - `BADGE_MIND` (4) may only turn the maintenance mode on. Nothing holds it yet.
- **`init`:**
  - It reads `data/mode.conf` once the boot volume is mounted. The words are `development` and `maintenance`; any other word is logged and left out.
  - `set-mode` writes the file whole, then logs `[INIT] MODE <WHICH> ON|OFF BY operator|mind (PID n, BADGE b): <reason>`.
  - A refusal is logged with the caller's PID and badge.
- **The Effector services** start and stop with the modes once they are boot images (750-KRN-0119). Until then `init` logs that this build has none.
- **Checked:**
  - `check services` reads the modes, and is refused a change with its lifecycle client;
  - a QEMU suite `modes` passed locally on x86 and on aarch64.

## APP: the shell's `mode` command (750-EFF-0013's design, in place of 750-APP-0093's `effector enable|disable`)

- **The client:** `mind::idl::init::{mode, set_mode}` on `Endpoint(SLOT_MODE)`. The shell holds it from boot.
- **Turning a mode on** asks at the local console first; `reason` carries what the operator typed, or `operator`.
- **`mode` alone** prints `mode()`.
- **When your command exists,** my `modes` suite will drive `set-mode` through it for 750-KRN-0143's criteria.

## EFF

750-KRN-0143's "Settled" section records the maintainer's answer on the safe start, and the issue has a criterion for it. A safe start does not yet start the services, because they are not boot images.

## ALL: ABI 14

The new slot changes the ABI, so everything is rebuilt together, as for every ABI step. It reaches `main` through the landing queue. I will say so when it lands.
