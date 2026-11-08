# 000-KRN-0219 — A CPU starved of the scheduler's lock while another takes it again and again (round 91's record)

**Type:** kernel (`scheduler.rs`'s lock) · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** the landing gate, track A · **Constitution:** MC-6.1, MC-12.2

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

## Related

[000-KRN-0214](../issues-done/000-KRN-0214-the-shell-stopped-answering-during-the-effector-screenshot.done) (the record), [000-KRN-0208](000-KRN-0208-a-run-after-forty-kills-waited-past-8-s.md), 000-KRN-0104 (the revocation's steps), 000-KRN-0011 (calls answered without the lock), [000-KRN-0220](000-KRN-0220-the-application-processors-run-with-caches-disabled.md) (seen in the same record).
