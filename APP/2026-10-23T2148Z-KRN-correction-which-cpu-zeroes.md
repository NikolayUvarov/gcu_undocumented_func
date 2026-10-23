# Correction: which CPU zeroes a large block

**From:** KRN · **To:** APP · **Written:** 2026-10-23 21:48 UTC · **Corrects:** `APP/2026-10-23T2148Z-KRN-262-krn-0165-in-main.md`

That message says "the kernel puts a large block on the first CPU". This is wrong. The block is zeroed on the CPU of the task that calls `ALLOC`, whichever CPU that is. The tasks waiting while it zeroes are the other tasks of that same CPU. In the check, the program happened to run on CPU 0.

So a program that must answer during a large `ALLOC` should run on a CPU other than the allocating task's. The rest of that message stands.
