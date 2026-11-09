# 000-KRN-0219 — A CPU starved of the scheduler's lock while another takes it again and again (round 91's record)

**Type:** kernel (`scheduler.rs`'s lock) · **Owner:** kernel session · **Priority:** P1 · **Status:** open (the ticket lock refused by round 95's gate; a test-and-set lock with a turn for a CPU that waited long made and tested on the host and in QEMU, below; the gate and the 10 runs beside a busy lane left) · **Blocked by:** — · **Roadmap:** the landing gate, track A · **Constitution:** MC-6.1, MC-12.2

## Problem

**The fourth time** the shell echoed a command and started nothing (000-KRN-0214's three, 000-KRN-0208's), and **the first with a record** of every CPU (000-KRN-0214's step 1).

- **Where:** round 91's gate, candidate 746430f6, x32-128, lane `x32`, `x86: network, TLS and Effector`, after 116 s.
- **What ran:** the `net` suite's `flow_bounds_check`.
  - `brief`, a copy of `netcheck`, held a connection past its grant's 10 s, until `NETCHECK hold:… NO GRANT`.
  - The shell's next command, `dmesg -s netpolicy`, was echoed, and no `STARTED` line came before the 8 s wait ran out.
- **The record** (`/home/gater/mind-core/KRN/logs/land-746430f6/x32-03/mind-core-stuck-1421505-1.log` on x32-128). Resolved against that run's own `kernel.elf` (base `0x1CFC7000`, found from the IDT's address):

  | CPU | RIP | where | RFLAGS | state |
  |---|---|---|---|---|
  | 0 | `0x1CFE6E92` | `kernel::scheduler::interrupt` +0x172: the `pause` loop of `locked`, waiting for `kernel::scheduler::LOCK` | `0x02` (IF clear) | running |
  | 1 | `0x1CFDB150` | `Scheduler::syscall` +0x890, after `paging::Space::find`, inside the lock | `0x46` (IF clear) | running |
  | 2, 3 | `0x1CFE2661` | `kernel::scheduler::idle`, after `hlt` | `0x246` | halted |

  CPU 0 and CPU 1 ran different address spaces (`CR3` `0x1C1D7000` and `0x1C1FD000`).

**What it shows, from one snapshot.** One CPU waited for the lock while another held it in a system call, and the two others slept.

**A reading, not a cause yet (MC-12.2).**
- **The lock** (`LOCK`, `locked()`) is a test-and-test-and-set: whoever sees it free first takes it, in no order.
- **A task that enters system calls one after another** takes it again each time it lets go, and a CPU waiting in `interrupt` can lose every race for as long as that lasts.
- **The kernel knows the pattern already.** The revocation loop spins `LET_GO_SPINS` before it takes the lock again, so that "a CPU waiting for the lock takes it now: a test-and-set lock would otherwise go back to this one" (000-KRN-0104). Nothing else does that.
- **It fits what was seen:**
  - every case had a busy neighbour lane, so QEMU's vCPU threads ran at different speeds;
  - the program that stopped was always the shell's start of another (`loader`, on whichever CPU lost);
  - the time was a grant's expiry here, when `netpolicy`, `netstack` and the holder all make calls.

**The group passed when the gate ran it again alone** (1097 s), so round 91 landed. The record is the evidence to keep: `/home/gater/mind-core/KRN/logs/land-746430f6/x32-03/` on x32-128.

## Plan

1. **A lock no CPU waits behind for long.**
   - ~~A ticket lock~~ (refused by round 95's gate, below).
   - **The lock:** test and test-and-set, so whichever waiting CPU runs when the lock is let go takes it.
     - A waiter that has spun `PATIENCE` (1024) turns marks itself next. No other CPU tries the lock while it is marked; it takes the lock, then clears its mark.
     - **Unless the marked waiter is not running:** another waiter that sees the lock free under another's mark `ABSENT` (256) turns in a row takes it, and the mark stays. A running marked waiter takes a free lock within a turn or two.
     - The mark is drawn from a counter, so no two waiters share one.
   - **Why it does not crawl as the ticket lock did:**
     - a waiter marks itself only while it runs, one at a time, and a mark whose vCPU the host stopped holds a free lock up for `ABSENT` turns only;
     - the ticket lock fixed the order when each CPU drew its ticket, so every handoff waited for whichever vCPU was next to be run by the host.
   - `LET_GO_SPINS` then has nothing to do and goes.
   - The lock is taken with interrupts off, as now (`interrupts::without`), and never twice by one CPU: the kernel does not nest `locked`.
2. **Host tests:**
   - a marked waiter is let in before the holder takes the lock again;
   - beside the test-and-set and ticket locks: one thread taking the lock in a tight loop while another waits, and more threads than CPUs.
3. **In QEMU:** `kbench`'s lock rows before and after on 4 and 16 CPUs (000-KRN-0011's measure), and the `net` group repeated beside a busy lane (000-KRN-0214's 10 runs).
4. **If the wait comes back** with the fair lock, the record names the CPU and the task again, and this reading was wrong.

## Acceptance criteria

- The host tests show a marked waiter let in before the holder takes the lock again, and print the three locks beside each other.
- The gate passes; `kbench`'s numbers on 4 and 16 CPUs are recorded before and after.
- 10 runs of `x86: network, TLS and Effector` pass in a row on x32-128 beside a busy lane, and that is recorded.

## Round 95's gate (2026-11-09): the ticket lock refused

- **Where:** candidate 86e7ac21 (this task and 100000-PRT-0012), `aarch64: 16 CPUs` on acc-2 (8 host CPUs, beside the other lanes).
  - It hung twice: in its lane after 95 s, and when run again alone after 110 s.
  - Both times it stopped after `run clock &`, in `clocks_on_many_cpus`, the 23rd clock.
  - Logs: `/home/un/mind-core/KRN/logs/land-86e7ac21/acc2-02` and `acc2-95` on acc-2.
- **The record** (`mind-core-stuck-3039830-1.log`):
  - every one of the 16 CPUs was at the same PC, the ticket lock's wait, with interrupts off;
  - their tickets ran from `0x3eef3` to `0x3ef02`, one each;
  - the others had read `serving` as `0x3eef3`;
  - the CPU holding ticket `0x3eef3`, CPU 0, had last read `0x3eef0`: it had not run for three turns.
- **The reading.**
  - The turn belonged to a vCPU the host was not running at that moment, so every other CPU waited for the host to run it.
  - A ticket lock fixes the order when each CPU draws its ticket. With more vCPUs than host CPUs, each handoff can wait for a host scheduling turn.
  - Linux falls back to test-and-set under a hypervisor for the same reason (`virt_spin_lock`).
- **Reproduced here,** in the kernel session's container (4 CPUs): the image of 86e7ac21, `--cpus 16 --suites normal`, stopped during boot after 90 s, with nine CPUs in the same wait and the same pattern.
- **The candidate was returned** (`LANDING/2026-11-09T0100Z-KRN-round-86e7ac21.md`).

## Progress (2026-11-08): the ticket lock, since refused

- **`common/ticket.rs`** (since removed): the ticket lock, `next` and `serving` counters.
- **`scheduler.rs`:**
  - `LOCK` is one, taken in `locked` with interrupts off as before;
  - the revocation loop's `LET_GO_SPINS` and its spin are gone: a CPU that waits is served before this one takes the lock again.
- **`tests/ticket_host.rs`** (since replaced), run by `scripts/host_tests.sh`:
  - mutual exclusion: four threads, 80 000 increments, none lost;
  - fairness: one thread takes the lock in a tight loop with a short hold while another takes it 2000 times. With the ticket lock the busy one took it at most once while the other waited for a turn.
  - The test-and-set lock's count is printed beside it: **up to 37 347** in the kernel session's container, 4 CPUs. That is the starvation the record suggested, shown on the host (it does not prove the record's cause, MC-12.2).
- **`kbench syscall ipc caps timer --quick` on x86,** the kernel session's container (QEMU 8.2.2, TCG, one run each; QEMU's times are not a machine's, MC-12.1).
  - **The old lock** was measured on the image of `claude/550-NET-0061-the-radio-restarted` (main 85471bfe), **the ticket lock** on `claude/000-KRN-0219-ticket-lock` (main ef12792c).
  - Medians:

    | Row | 4 CPUs, before | 4 CPUs, after | 16 CPUs, before | 16 CPUs, after |
    |---|---|---|---|---|
    | uptime (empty call) | 3.32 µs | 2.77 µs | 3.43 µs | 2.51 µs |
    | call and reply | 177 µs | 215 µs | 186 µs | 166 µs |
    | mint and drop | 7.08 µs | 6.49 µs | 6.43 µs | 11.0 µs |
    | revoke 1000 nodes | 1.94 ms | 1.08 ms | 2.51 ms | 646 µs |
    | revoke 10000 nodes | 15.1 ms | 4.92 ms | 14.1 ms | 5.12 ms |
    | sleep 10 ms | 11.4 ms | 10.1 ms | 10.8 ms | 10.9 ms |

  - The revocations are two to four times faster, as their steps no longer spin `LET_GO_SPINS` before taking the lock again. The other rows move both ways within TCG's noise.
- **Passed with it** in the same container: x86 `normal` (4 CPUs) and aarch64 `smp`.
- **Left then:** the gate, which refused it (above).

## Progress (2026-11-09): test-and-set with a turn for a CPU that waited long

- **`common/spinlock.rs`:** the lock of plan step 1, `PATIENCE` 1024 turns and `ABSENT` 256; `common/ticket.rs` and its test are gone.
- **`scheduler.rs`:** `LOCK` is the new lock, taken in `locked` as before; `LET_GO_SPINS` stays gone.
- **`tests/spinlock_host.rs`,** run by `scripts/host_tests.sh`. The test-and-set and ticket locks are kept in the test only, to be measured beside it.
  - **Exclusion:** four threads, 80 000 increments, each waiter marked next after 8 turns: none lost, no mark left.
  - **A marked waiter goes first:** the holder lets go and asks again at once, and the marked waiter is let in before it.
  - **An absent mark:** with a mark no waiter holds, another takes the free lock after `ABSENT` turns, and the mark stays.
  - **Printed, not asserted:** three runs on 02cbda5f in the kernel session's container (4 CPUs). They show the two failures, not a cause (MC-12.2).
    - **One thread taking the lock again and again** while another waits for one turn: the busy one took it at most 23 to 304 times with this lock, 13 181 to 39 358 with test-and-set, and 5 to 26 with the ticket lock.
    - **16 threads on 4 CPUs for 0.5 s:** 165 691 to 207 387 turns with this lock, 248 816 to 294 498 with test-and-set, and 3 160 to 4 390 with the ticket lock, 40 to 65 times fewer: its turns wait for the host to run the one thread next.
- **In QEMU, on 811b634d** (the lock without the `ABSENT` rule):
  - **In the kernel session's container** (4 CPUs, TCG, QEMU 8.2.2):
    - `aarch64 --cpus 16 --suites normal` passed in 13 min 41 s, where the ticket lock's image stopped during boot;
    - x86 `normal` (4 CPUs) passed, and aarch64 `smp`.
  - **On acc-2** (8 CPUs, alone), `aarch64: 16 CPUs` three times:
    - runs 2 and 3 passed, in 339 s and 320 s;
    - run 1 failed after 791 s: after the memory refusal, with 156 clocks drawing, `date` answered at the edge of the harness's 8 s wait (`DATE: … 01:41:48`, the wait ran out at 01:41:49).
  - **Run 1's record** (`/home/un/mind-core/KRN/logs/k219-811b634d/run-1/` on acc-2), resolved against the kernel of that commit (base `0x571A4000`):
    - CPU 8 held the lock, in `Scheduler::select` called from `interrupt`, with 171 tasks;
    - CPU 2 was marked and running: the last `next` it read was its own mark;
    - eleven others waited in `interrupt`'s inlined `lock_with`, from half a million to 10 million turns;
    - some had last read a mark 16 marks old: their vCPUs had not run since;
    - about 688 000 marks had been drawn.
  - **Its reading** (MC-12.2): no deadlock. One global lock was saturated by 16 CPUs and 171 tasks, as the test's own comment on a hundred clocks drawing every second expects.
    - A marked waiter whose vCPU the host stops leaves a free lock no other takes, so the `ABSENT` rule followed (02cbda5f).
    - The old lock took 1292 s for the same group on acc-2 in round 94's gate, and passed.
  - **`kbench syscall ipc caps timer --quick` on x86,** the container (TCG, one run each; QEMU's times are not a machine's, MC-12.1), medians:

    | Row | 4 CPUs | 16 CPUs |
    |---|---|---|
    | uptime (empty call) | 3.18 µs | 2.43 µs |
    | call and reply | 173 µs | 171 µs |
    | mint and drop | 6.78 µs | 6.49 µs |
    | revoke 1000 nodes | 1.36 ms | 1.46 ms |
    | revoke 10000 nodes | 5.12 ms | 6.34 ms |
    | sleep 10 ms | 10.9 ms | 10.9 ms |

    The revocations stay as fast as under the ticket lock; the old lock's were two to three times slower (the table above).

## Related

[000-KRN-0214](../issues-done/000-KRN-0214-the-shell-stopped-answering-during-the-effector-screenshot.done) (the record), [000-KRN-0208](000-KRN-0208-a-run-after-forty-kills-waited-past-8-s.md), 000-KRN-0104 (the revocation's steps), 000-KRN-0011 (calls answered without the lock), [000-KRN-0220](000-KRN-0220-the-application-processors-run-with-caches-disabled.md) (seen in the same record).
