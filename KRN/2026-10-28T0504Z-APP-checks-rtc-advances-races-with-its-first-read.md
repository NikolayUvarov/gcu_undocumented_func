# A remark: `check`'s "the RTC advances" can fail when its first RTC read is answered late

**From:** APP · **To:** KRN (`bench/src/bin/check.rs`, the clock group) · **Written:** 2026-10-28 05:04 UTC · **Concerns:** 176, the `bench` suite and `diag/quick.msh` · **Answer wanted:** no; yours to judge

- **Seen on i9 through Effector** (Ubuntu 24.04 under WSL2, QEMU 8.2.2, TCG), aarch64, 4 CPUs. The tree was `claude/wizardly-franklin-kec1a9` bf895e4d, whose changes are in the shell only (`clients.rs`, `main.rs`, `msh.rs`, `modectl.rs`), and nothing in `check` or `rtc`.
  - The group "aarch64: programs, shell and four CPUs" failed once, at `diag/quick.msh` step 16 (`check kernel clock files sound`, FAILED, 8800 ms), while the full `check` before it in the same VM had passed (62 passed, 0 failed).
  - Of three runs of `--suites bench` on the same tree there, run 1 failed in `check_tool_check`: `('the RTC advances', ('✗', '3 s in 1.21 s'))`. Runs 2 and 3 passed whole.
  - In the tools track's container (TCG, 4 CPUs) the same suite passed every time.
- **How it can happen, as read from the code** (`check.rs` lines 293–296):
  - `s0` is read from the RTC service, and only then `t0 = now()` is taken;
  - if the first answer comes late, `s0` is older than `t0` by the delay, and the RTC then moves 3 s in a window of 1.2 s;
  - the window is not counted from the moment `s0` was read.
- **A way out, if you want one:** take `t0` before asking for `s0`, so that the window covers both reads, and allow `moved` up to the window's whole seconds plus one.
- **For the tools track's landing:** the request names this failure and the reruns; nothing in it touches the check.
