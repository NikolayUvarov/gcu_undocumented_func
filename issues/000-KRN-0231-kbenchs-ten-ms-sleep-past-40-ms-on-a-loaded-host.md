# 000-KRN-0231 — `kbench`'s 10 ms sleep past 40 ms on a loaded host

**Type:** tests (`kbench_check`) and the kernel's sleep · **Owner:** `KRN` · **Priority:** P2 · **Status:** open (cause found and fixed on `claude/000-KRN-0231-a-ten-ms-sleep`, 2026-11-10; the gate and the ten runs on acc-2 left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track A · **Constitution:** MC-12.1, MC-12.2

## Problem

`kbench_check` (`tests/qemu_smoke.py`, the `bench` suite in "x86: keys, shell, tools") asks that `sleep 10 ms`'s median be at most 40 ms. On 2026-11-10 it failed four times:

| when | tree | machine | `sleep 10 ms` median / longest | `sleep 1 ms` median |
|---|---|---|---|---|
| round 112's gate | candidate a9132d66 | i9 | 47.7 / 67.7 ms | 1.85 ms |
| round 112, alone | candidate a9132d66 | i9 | 47.7 ms | — |
| by hand | `main` 06f3a8b0 | i9 | 47.0 / 66.2 ms | 1.92 ms |
| round 114's gate | candidate 60ba280e | acc-2 | over 40 ms (the line was cut) | — |

- **The same groups passed elsewhere:** on acc-1 (477 s, round 112's candidate), on acc-2 when round 114's gate ran it again alone (519 s), and on i9 in rounds 102 to 110.
- **So a 10 ms sleep takes 40 to 70 ms in QEMU on some hosts at some times,** while a 1 ms sleep stays near 2 ms.

**Not known:** whether the time is the host's (QEMU's timer under TCG on a busy or coarse host) or the kernel's (how a sleep longer than a tick is woken). This is not a root cause (MC-12.2).

## Plan

1. **The kernel's side.** Read how a sleep of more than one tick is armed and woken on x86 (the 1 ms tick, 000-KRN-0083). Then time it in QEMU on an idle host and beside a loaded one: the requested sleep, the tick count, the wake.
2. **Fix what is the kernel's.** If a sleep of n ticks waits for later ticks than the n-th, fix it. If the host's timer is what lags, the check keeps its bound and the gate keeps such a host out, saying so in `LANDING/`, as round 113 did with i9.
3. **The bound is not widened** to make a run pass (MC-12.5).

## Acceptance criteria

- The cause is named, with the measurements that show it.
- "x86: keys, shell, tools" passes ten times in a row on acc-2 beside a loaded lane.

## Progress

**The cause (2026-11-10).** The sleep is armed and woken on time; the CPU it wakes on is held by the compositor.

- **Measured in this container**, QEMU 8.2.2, TCG, 4 CPUs, the `bench` suite, `main` 49282594. A temporary build printed each of `kbench`'s samples and, for each of its wakes chosen more than 5 ms after the tick made it ready, the CPU and what had run there. Times in ms:

  | build | `sleep 10 ms`, 50 samples | `sleep 1 ms`, 100 samples |
  |---|---|---|
  | `main` | 25 between 10.1 and 10.9, 25 between 39.3 and 65.0 | 86 near 2, 14 between 36.5 and 38.4 |
  | the wake IPI sent whatever `woken` says (a shorter run) | 3 of 5 past 41 | 1 of 10 at 50.7 |
  | the compositor's hold (000-KRN-0086, 0186) off, two runs | 25 between 10.1 and 12.1, 25 between 40.4 and 75.2, each run | 88 between 1.1 and 2.3, 12 between 37.6 and 67.8, each run |
  | the first fix (2cc88f4a, below) | all 50 between 10.1 and 11.3, median 11.0 | all 100 between 1.6 and 2.1 |
  | a move for good (d0f292c8, below) | all 50 between 10.0 and 11.2, median 11.0 | all 100 between 1.0 and 2.7 |
  | **the fix below, two runs** | **all 50 between 10.5 and 13.1, and between 10.2 and 12.8; median 11.0 each** | **all 100 between 1.1 and 2.6, and between 1.0 and 2.1** |

- **Every late wake** (51 in one run) was on CPU 0, and the task that ran there meanwhile was the compositor (50 of 51; `tls` once). In the 40 lines read, no other task was ready there.
- **The chain:**
  - every sleep marks the foreground task as drawn (`dirty`), so the compositor copies its frame;
  - under TCG a copy runs 30 to 60 ms;
  - the compositor is in the system band, which runs before the application band on its CPU;
  - `kbench` was placed on the compositor's CPU: applications were balanced by application count alone, and a tie went to CPU 0.
- **A loaded host** makes the copy slower, so the median passed 40 ms on some hosts only.
- **With one CPU,** five samples of `sleep 10 ms` were on time (10.3 to 11.5).

**A first fix, replaced (2cc88f4a).** It placed a new application, of the CPUs with the fewest, on the one whose services had run least.
- `kbench` was on time with it (its row above).
- The `nnd` suite then failed. `nncheck`'s cancel of a 131 ms job came after the job had ended in all 16 tries, even at 0 ms.
- **Why, as read from the code (not measured):** `nnd`'s worker is a service, placed by service count alone. It can go to the CPU `nncheck` was given, and `nncheck` then waits behind it, as `kbench` had behind the compositor. The fix below passes the same check.
- A placement made at the start cannot know which service will be busy later.

**Two more tries, replaced (d0f292c8 and after, in this container's x86 groups):**
- **A move for good:**
  - An application ready behind a system-band task moved to an idle CPU and stayed there.
  - **The kernel heap check failed:** "task teardown leaked resources", in the `normal` suite with 16 CPUs and in the `keys` suite, on i9 too. The old CPU's list kept the slot and its room, which reap does not trim.
  - **Then the `busy` suite failed:** its CPU-bound loop moved, and the check, which reads the loop's CPU, read 0.56 of the time other tasks leave.
  - **Then the `heap` suite's spread failed:** its four stress programs no longer covered every CPU, because an application that moved for good changed where spawn's balance put the next ones.

**The fix (`kernel/src/scheduler.rs`: `move_waiting`, `relist`, and the return in `select`).** An application whose wait ended borrows an idle CPU while its own runs a system-band task, and goes back when it waits again.
- **Who borrows:** an application whose wait ended (a sleep, a message, a reply), ready on a CPU that runs a system-band task.
- **Which CPU:** an idle one, if one is online and holds no application.
  - It runs there until it waits again, then goes back to its own CPU (`home`).
  - `ps` and `stat` show `home`, and spawn's balance counts it.
- **When it is looked at:**
  - when the task becomes ready;
  - when a `select` puts a system-band task first;
  - at each slice's end.
- **What is not moved:**
  - a task still ready when its CPU last switched from it (preempted), or one that has not run yet;
  - a worker, which stays where 262-KRN-0163 placed it;
  - a task that runs, is held for the compositor's copy, or has no budget left;
  - a service.
- **Each CPU's list stays sized to its tasks,** as spawn and reap keep it, so the kernel heap returns as tasks end.
- **Unchanged:** the band order (the system band still runs first on its CPU, MC-5.5), the sleep, and `kbench_check`'s 40 ms bound (MC-12.5).
- **Why the move is safe:** each switch reloads the address space and flushes that CPU's TLB (no PCID on x86, no ASIDs on aarch64). A task's registers and FP state are saved in the task at each switch on both architectures.

**With it,** in this container, x86, 4 CPUs:
- "x86: scheduling, isolation, heap", "x86: keys, shell, tools", "x86: 16 CPUs" and "x86: one CPU" all passed;
- `kbench`'s `sleep 10 ms` median was 11.0 ms;
- `nncheck`'s cancel came partway;
- the busy loop took 1.00 of the time other tasks leave its CPU;
- the heap suite's stress programs were on all four CPUs.
- **A third run of the `bench` suite** failed `kbench_check`'s bound on the empty call, 10.3 µs against 10 µs; the two runs after it gave 2.8 and 4.6 µs. Nothing of this change is on that call's path, which takes no lock. It is noted as seen once.
- **One number to watch:** the revocation check's longest 10 ms sleep was 77 ms in this run, where two runs before the change gave 14 ms. Its bound is 1000 ms; one sample says nothing yet.

**Not done:**
- An application behind another application is not moved; the slices share that CPU, as before.
- With no idle CPU free of applications, an application still waits behind the system band.

## Related

000-KRN-0065 (a sleep never shorter than asked), 000-KRN-0083 (the 1 ms tick on x86), round 112's and round 114's gate logs, `HUMAN/2026-11-10T1426Z-KRN-i9s-timer-…`.
