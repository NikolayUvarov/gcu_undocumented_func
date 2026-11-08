# KRN ready: the scheduler's lock is a ticket lock; the A64's counter erratum worked around

**Ready:** `claude/youthful-mendel-mf1soy` 86e7ac21

**From:** the kernel session (KRN, PRT)

**Advances:**
- **000-KRN-0219.** `scheduler.rs`'s lock becomes a ticket lock (`common/ticket.rs`), each CPU served in the order it asked, and the revocation loop's `LET_GO_SPINS` goes.
  - The host test shows the starvation: while one turn was waited for, a busy thread took the old lock up to 37 347 times, the ticket lock once at most.
  - The 10 runs beside a busy lane on x32-128 come after the round.
- **100000-PRT-0012.** Every read of the counter goes through `clock::count()`, which retries a read whose low 9 bits are all ones or all zeros when the device tree names `allwinner,erratum-unknown1`.
  - `common/counter.rs` holds that read, with its host test.

**Tested** in the kernel session's container (Ubuntu 24.04.5, QEMU 8.2.2, TCG):
- the host tests `ticket`, `counter`, `fdt`;
- with the ticket lock: x86 `normal` and aarch64 `smp`;
- with the counter's change: aarch64 `smp`, and a boot under U-Boot with a `virt` tree naming the erratum (the retry on, four CPUs, the clock right);
- `kbench` before and after on x86, 4 and 16 CPUs (in 0219's issue).

The full gate is the round's.
