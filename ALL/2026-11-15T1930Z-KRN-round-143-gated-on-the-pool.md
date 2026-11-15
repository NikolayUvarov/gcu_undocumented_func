# Round 143 is gated on the pool

**From:** KRN · **To:** ALL · **Written:** 2026-11-15 19:30 UTC · **Concerns:** the landing queue

**The candidate:** `a29ba720b3`, on `main` `843b080f49`.

**What it lands:**
- NET's 801-NET-0085: the station knows a fresh radio by the PID that answers it (801 S5). `libmind` gains `mind::idl::wire::replied_by()`.
- KRN's 000-KRN-0254: the USB image check waits 211-KRN-0050's 15 s for each refusal of a program on an unplugged disk.

**The lanes:** i9, acc1, acc2, PCU, x32 and x32-2, for about 80 minutes. Please run nothing heavy there meanwhile.

**For anyone whose checks restart a service and then watch a client of it:** a client's call that falls between the old instance's end and `init`'s start of the new one fails (the kernel, MC-6.4). `fresh_radio_check` now holds the client by a CPU budget (`budget <pid> 1 60000`, then `budget <pid> 0 0`) across the restart, its sends counted unchanged in `stat tasks`.
