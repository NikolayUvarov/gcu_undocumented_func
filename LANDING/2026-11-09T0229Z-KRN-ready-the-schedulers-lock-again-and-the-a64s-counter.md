# KRN ready: the scheduler's lock, test-and-set with a turn for a CPU that waited long; the A64's counter erratum

**Ready:** `claude/youthful-mendel-mf1soy` 5bc2ce6e

**From:** the kernel session (KRN, PRT)

**Replaces** `2026-11-08T2338Z-KRN-ready-the-ticket-lock-and-the-a64s-counter.md`, returned in round 86e7ac21: `aarch64: 16 CPUs` hung twice on acc-2 with every CPU in the ticket lock's wait.

**Advances:**
- **000-KRN-0219.** `scheduler.rs`'s lock is `common/spinlock.rs`; the ticket lock is gone.
  - It is test and test-and-set. A waiter that has spun 1024 turns marks itself next, and no other tries the lock before it has taken it.
  - Unless the lock stays free 256 turns in a row under the mark: then the marked waiter's vCPU is not running, and another takes the lock, the mark kept.
  - The revocation loop's `LET_GO_SPINS` stays gone.
- **100000-PRT-0012**, unchanged since the returned request: every counter read goes through `clock::count()`, with the A64's retry when the device tree names `allwinner,erratum-unknown1`.

**Tested:**
- **On acc-2, alone,** `aarch64: 16 CPUs` three times on 02cbda5f (this lock's kernel): 216 s, 215 s, 211 s. Main's old lock, three times beside it: 210 s, 213 s, 598 s.
- **In the kernel session's container** (Ubuntu 24.04.5, QEMU 8.2.2, TCG, 4 CPUs):
  - `aarch64 --cpus 16 --suites normal` in 4 min 23 s; the ticket lock's image stopped during boot there;
  - x86 `normal`, aarch64 `smp`, and the host tests `spinlock`, `counter`, `fdt`;
  - `kbench` on x86 with 4 and 16 CPUs (in 0219's issue).

The full gate is the round's.
