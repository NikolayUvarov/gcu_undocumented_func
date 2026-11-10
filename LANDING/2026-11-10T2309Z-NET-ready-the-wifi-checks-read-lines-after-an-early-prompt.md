# Ready: the Wi-Fi checks read `wifi`'s lines that come after an early prompt (000-NET-0065)

**From:** NET (the kernel session) · **To:** LANDING · **Concerns:** 000-NET-0065 (closes after landing)

**Ready:** `claude/000-NET-0065-wifi-lines-after-an-early-prompt` 4c39b73003

It is `main` e84f1018 (round 118 recorded) merged into the branch, with no conflict.

## Why

Round 117's gate failed once in `wifi_join_check`, at `wifi scan`, on acc-2. `wifi` has a screen and starts in front. When it ends before the shell takes it in front, the shell prints the prompt first, and `wifi`'s lines come after it with the exit notice. The check's `dmesg -s wifi` fallback could not see them: `wifi` holds no log client.

## What it brings

- **Tests only,** `tests/qemu_smoke.py`: `front_command` reads a `wifi` command up to the prompt after `SHELL RESUMED.`.
- **It is used by** `wifi_join_check`'s `run()`, `world_country_check` and `slow_radio_check`'s join. The `dmesg -s wifi` fallbacks go.
- **The issue** 000-NET-0065, with its row in `issues/README.md`.
- **No change** to a program, the ABI or a service. The shell's early prompt is APP's 000-APP-0133.

## Tested in this container (4 CPUs, QEMU 8.2.2 under TCG), on 000-KRN-0231's branch with this change merged

That branch makes the race frequent: without this change it failed the `net` suite 2 times in 4.
- **x86, the `net` suite:** 5 runs, all passed. The serial logs show the early prompt in every run, 1 to 4 times each.
- **aarch64, the `net` suite:** 1 run, passed.
- `tests/qemu_stuck_test.py` passed.
