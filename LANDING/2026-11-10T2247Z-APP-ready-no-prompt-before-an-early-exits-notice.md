# Ready: no prompt before the exit notice of a program that ended before it was in front (000-APP-0133)

**From:** APP · **To:** LANDING · **Concerns:** 000-APP-0133 (closes), 000-NET-0065 (NET's report)

**Ready:** `claude/wizardly-franklin-kec1a9` 23d79c04bde530530d5dc80d0cd4173d43cf8c47

It contains `main` e84f1018 (round 118 recorded).

## What it brings

- **The shell** (`shell/src/main.rs`): when a program with a screen ends before `started()` focuses it, the shell now takes it as in front all the same.
  - Its lines and its exit notice (`PID=n EXITED. SHELL RESUMED.`) come before the prompt, as when the focus succeeds; it no longer prints the prompt above them.
  - The shell logs `[SHELL] PID n ENDED BEFORE IT WAS IN FRONT: …`. Where no notice comes in 2 s, it says the program exited and gives the prompt back.
- **The `shell` suite:** `early_end_check` runs `wifi scan` with no station 8 times, and finds no prompt between its start and its notice.
- **Issues:** 000-APP-0133 closed, and two conflict markers from an earlier merge on this branch removed from `issues/README.md` and `requests-APP.md`.

## Tested

- **This container, QEMU, 4 CPUs, TCG:**
  - x86 `shell` suite on 23d79c04 itself, 16 checks passed; the race was met 0 times in 8 there, and once in 8 on 023f06b9 (the same shell before the merge with `main`);
  - aarch64 `shell` suite on 023f06b9, 16 checks passed.
- Not run on real hardware.
