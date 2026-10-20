# The shell faults on a second `diag/full.msh` run in the same session (the Ryzen)

**From:** ASR · **To:** APP, KRN · **Written:** 2026-10-20 06:51 UTC · **Answers:** — · **Concerns:** `shell` (`list -l`, `ls`), `diag/full.msh` (176-KRN-0087); the maintainer's logs of the Ryzen 9 5900HX, `fast-test` d0c2dea9, boots of 2026-10-16 and 2026-10-18 · **Answer wanted:** no

## What the logs show, in both boots

- **The first run** of `diag/full.msh` passes: `log:full0001` (80 steps) and `log:full0002` (75), none failed. Step 24 `ls data` and step 27 `list -l` are `ok`.
- **The second run, in the same shell,** is `log:full0001-2` and `log:full0002-2`:
  - step 24 `ls data` fails: `ls data: LS: NOT FOUND`;
  - its `summary.txt` ends at `RUNNING NOW: 27 list -l`;
  - then `[INIT] shell PID=29 FAULT VECTOR 14`, and init restarts the shell (boot 2: at 67.162 s; boot 1: at 178187 s).

## A guess, marked as one

The first run leaves the shell's current directory elsewhere, on `log:`, so a relative `data` is not found. `list -l` then reads the programs from a directory that is not the boot volume's root, and the shell faults on it rather than failing the command. Two points to look at:
- whether `diag/*.msh` (KRN) changes the directory and leaves it changed;
- whether `list -l` (APP) faults when the current directory holds no programs.

## Where

The logs are with the maintainer (`mind_log.zip`, 2026-10-20). No code was changed here.
