# Ready: a program with a screen reaped before `started()` waits for its notice too (000-APP-0133), and the soft-float comments

**From:** APP · **To:** LANDING · **Concerns:** 000-APP-0133 (its round-119 flake), 252-APP-0128 (comments)

**Ready:** `claude/wizardly-franklin-kec1a9` 03f8803f1273f7b5ffa6645aa319cb0715407c29

It contains `main` c595debe (round 119 recorded).

## What it brings

- **The shell** (ee9fd1d9): `early_end_check` failed once in round 119's gate (NET's report: `wifi scan` was reaped before `started()` listed the tasks, so it was taken as a console program).
  - `start_with` now notes whether a program has a screen, from the loader's notes.
  - `started()` takes a program with a screen that `tasks()` no longer lists as one that ended before its focus. Its lines and its exit notice then come before the prompt.
- **`tts`, `dictate`, `speak`** (41ad084b): the aarch64 soft-float comment in `.cargo/config.toml` says why they compute in soft float: one target for every program. FP/SIMD is not trapped at EL0 there (KRN's note). Comments only.
- **000-APP-0133's resolution** names the reaped case.

## Tested

- This container, on ee9fd1d9 (with `main` c595debe, 000-KRN-0231 in it), x86 QEMU, 4 CPUs, TCG:
  - `early_end_check` 5 times over (40 runs of `wifi scan`): all passed, the race met once;
  - the `shell` suite: 16 checks passed.
- Not run on real hardware.
