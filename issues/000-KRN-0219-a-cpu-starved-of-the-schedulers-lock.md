# 000-KRN-0219 — A CPU starved of the scheduler's lock while another takes it again and again (round 91's record)

**Type:** kernel (`scheduler.rs`'s lock) · **Owner:** kernel session · **Priority:** P1 · **Status:** open (the ticket lock made and tested on the host and in QEMU, below; the 10 runs beside a busy lane left) · **Blocked by:** — · **Roadmap:** the landing gate, track A · **Constitution:** MC-6.1, MC-12.2

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

1. **A fair lock.** `LOCK` becomes a ticket lock: a `next` and a `serving` counter, each CPU served in the order it asked, with `pause` while it waits. `LET_GO_SPINS` then has nothing to do and goes.
   - The lock is taken with interrupts off, as now (`interrupts::without`).
   - A CPU never takes it twice: the kernel does not nest `locked`, which a debug assertion checks.
2. **A test that shows the unfairness first:** a host test of the two locks under threads, one taking the lock in a tight loop, the other timing its waits. The test-and-set one's worst wait grows with the loop; the ticket lock's is bounded by one holder's hold.
3. **In QEMU:** `kbench`'s lock rows before and after on 4 and 16 CPUs (000-KRN-0011's measure), and the `net` group repeated beside a busy lane (000-KRN-0214's 10 runs).
4. **If the wait comes back** with the fair lock, the record names the CPU and the task again, and this reading was wrong.

## Acceptance criteria

- The host test shows the starvation with the old lock and none with the new one.
- The gate passes; `kbench`'s numbers on 4 and 16 CPUs are recorded before and after.
- 10 runs of `x86: network, TLS and Effector` pass in a row on x32-128 beside a busy lane, and that is recorded.

## Progress (2026-11-08)

- **`common/ticket.rs`:** the ticket lock, `next` and `serving` counters.
- **`scheduler.rs`:**
  - `LOCK` is one, taken in `locked` with interrupts off as before;
  - the revocation loop's `LET_GO_SPINS` and its spin are gone: a CPU that waits is served before this one takes the lock again.
- **`tests/ticket_host.rs`,** run by `scripts/host_tests.sh`:
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
- **Left:** the gate; 10 runs of `x86: network, TLS and Effector` on x32-128 beside a busy lane; `kbench` on the gate machines.

## Related

[000-KRN-0214](../issues-done/000-KRN-0214-the-shell-stopped-answering-during-the-effector-screenshot.done) (the record), [000-KRN-0208](000-KRN-0208-a-run-after-forty-kills-waited-past-8-s.md), 000-KRN-0104 (the revocation's steps), 000-KRN-0011 (calls answered without the lock), [000-KRN-0220](000-KRN-0220-the-application-processors-run-with-caches-disabled.md) (seen in the same record).
