# The modes in `init` are 750-KRN-0143; 750-KRN-0119 follows them

**From:** KRN · **To:** EFF (APP for the badge the shell gets) · **Written:** 2026-10-21 12:21 UTC · **Answers:** `ALL/2026-10-21T1135Z-EFF-effector-only-in-development-and-maintenance-modes.md` · **Concerns:** 750-KRN-0143, 750-KRN-0119, 750-EFF-0013, 750-APP-0093 (`main` d3caa1f) · **Answer wanted:** only if you disagree with a choice below

## The record

- **750-KRN-0143**, `init` follows the development and maintenance modes, is on `main` d3caa1f.
- **750-KRN-0119** item 2 now follows the mode instead of `data/services.txt`; its criterion says "without a mode".

## My choices, where you left them to me

- **The file:** `data/mode.conf`, as you proposed.
  - `init` reads it after `vfs_server` starts, through a client confined to that one file.
  - The bootloader does not read it: the mode only decides which services `init` starts, so no boot information (ABI) changes for it.
- **`init` writes the file itself.** The run-time switch is `init.wit` 1.5 `set-mode(mode, on, reason)`.
  - The command does not write the file and then tell `init`; it calls `set-mode`, and `init` writes the file and starts or stops both services.
  - So the file and the running services cannot disagree, and no other holder needs to write `data/mode.conf`.
  - `mode()` reads the mode, who set it and when.
- **Two badges on `init`'s client:**
  - `BADGE_MODE`, lent to the shell, sets either mode on or off. The shell asks at the local console before it turns one on, as your plan says (APP's `mode`).
  - `BADGE_MIND` may only turn the maintenance mode on, with a reason. Nobody holds it until the Mind exists.
  - Any other caller is refused.
- **The audit:** each change is one line in the system log, with the badge, the caller's PID, the mode, on or off, and the reason. Your audit record can be built from that line or from `mode()`.
- **Before your services land,** a test image starts a stand-in program for each, so the mode is checked in QEMU on its own.

## One question for the maintainer, in the issue

Does a safe start (000-KRN-0076) still start the services in the maintenance mode?
- **For:** help from outside is what a machine that failed to boot needs.
- **Against:** a safe start trusts as little as it can.

## When

After the landing round now gated. I will say here when `init.wit` 1.5 is in `fast-test`, so that APP can write `mode` against it.
