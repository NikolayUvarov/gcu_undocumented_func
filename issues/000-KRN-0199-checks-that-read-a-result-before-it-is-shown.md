# 000-KRN-0199 — Checks that read a result before a loaded host shows it: the colour after `fg`, a program's row after it ended, an idle CPU's halt

**Type:** tests · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** the landing queue (AGENTS.md section 4) · **Constitution:** MC-12.2

## Problem

In round 64 (candidate fbda1685), the group "x86: no PIT", the `normal` suite, failed on both lanes of `un-rizen-service`. The machine ran another track's suites and two gate lanes, with a load average near 58 on 16 cores. The group passed alone on `acc-1`.

- **On lane 2:** `fg must preserve state, not restart`. The suite reads the centre pixel at once after `FOREGROUND PID=3`. On a loaded host the compositor shows the program's frame later, and the pixel was still the other copy's colour. The two checks before it read the same way, one after a sleep of 150 ms.
- **On lane 1:** `assert set(task_rows(vm)) == {1, 3, 4, 5}` after `PID=2 EXITED. SHELL RESUMED.`. `ps` listed the ended program as `29 clock EXITED`: the boot CPU reaps a program that ended when it is idle, later under load (000-KRN-0168). `task_rows_reaped` waits for that, and the other suites use it already.
- **The same reading** follows a program's fault in the `isolation` suite (`{1}` after a fault), which has not failed yet.
- **In round 65** (candidate 370e905c), the group "x86: x2APIC" failed on `acc-2`: `CPU 0 did not halt when idle`, in the `smp` suite after `nnd_check`.
  - CPU 0 was at CPL 3, in a loop over two buffers 4 MiB apart with a count of `0x3E8000`, a 1280×800 frame: the compositor's copy of a whole frame (000-KRN-0086), drawing the last command's output.
  - The check samples each CPU 10 times, 20 ms apart: about 200 ms in all, short for a frame or two on that host. The `normal` suite's check of the boot CPU samples the same way.
  - Neither request of that round adds work in the guest: STO's adds the `blocks` program's assertions and a library, LNK's the host's side of the link.
- **In round 65 too,** the group "x86: no PIT" failed on `i9`: the `busy` suite's run-time check, `(5201, 5511, 1.11)`. It asked that the busy loop gain more than 30 % of the host's wall clock in a second. Round 64's "x86: scheduling, isolation, heap" failed the same way on `un-rizen` (121 ms of 1.19 s).
  - The loop (PID 28, band 1) was on CPU 0 with the compositor (PID 5, the system band). Right after `top` ends, the compositor redraws the screen there: the loop ran 39 slices, 310 ms, in 1103 ms of its own age.
  - The suite's later check already measures the loop's share against the time the other tasks leave its CPU (000-KRN-0026), and asks for more than 0.9 of it.

## Plan

- The three colour checks wait up to 5 s for the colour they want (`center_pixel_soon`), as the display check does already. A program that restarted, or input that did not reach it, shows a wrong colour that never changes, so the check still fails then.
- The four row checks after a program ended or was killed use `task_rows_reaped`.
- The x86 idle checks (`normal` for the boot CPU, `smp` for each CPU) sample `info registers` for up to 2 s until `HLT=1`. A CPU whose idle loop spins never shows it, so the check still fails then.
- The `busy` suite's first run-time check asks what the accounting itself must hold: the loop gains run time, and no more than its age gained plus a 10 ms slice. Its share of the CPU stays the later check's, against the time the other tasks leave.

## Acceptance criteria

- [ ] The `normal` and `isolation` suites pass on x86 with the waits, in QEMU locally and in a landing round's gate.
- [ ] Nothing waits where a wrong result could still turn right: each wait is for a colour, for a row of a program that has ended, or for `HLT=1`.
- [ ] No share of the host's wall clock is asked of the busy loop; its share of what its CPU leaves is still asked.

## Progress (2026-10-31)

- `tests/qemu_smoke.py`: the three colour checks of the `normal` suite and its two row checks; the `isolation` suite's two row checks after a fault; the two x86 idle checks; the `busy` suite's first run-time check.
- The `normal`, `isolation`, `smp` and `busy` suites passed locally (x86, QEMU TCG, 4 CPUs; `smp` with `-cpu max`, all CPUs `HLT`; `busy`: the loop took 1.01 of the time the other tasks left its CPU).

## Related

[000-KRN-0168](../issues-done/000-KRN-0168-the-suites-wait-for-an-ended-program-to-be-reaped.done), [000-KRN-0198](../issues-done/000-KRN-0198-two-waits-of-the-x86-suites-short-for-a-loaded-host.done).
