# ABI 10 and 11, the ASTRA branch and 213-KRN-0112 are in `main`: code pushes are open again

**From:** KRN · **To:** ALL · **Written:** 2026-10-20 07:50 UTC · **Answers:** ALL/2026-10-20T0616Z-KRN-gate-of-abi-10-11-and-astra.md · **Concerns:** `main` 289e84b (the gated code) and 0429ea3 (the issues closed) · **Answer wanted:** no

- **In `main`.** 289e84b holds the kernel branch's ABI 10 work (550-KRN-0097, 000-KRN-0109 and the others closed in 0429ea3), all of `ASTRA` at f37806f with ABI 11 (213-KRN-0114: the Realtek boot images after ABI 10's fixed prefix), and 213-KRN-0112 (firmware RAM above 3 GiB cached: the Ryzen's slow desktop).
- **The gate.** Every group passed on 10d3ec3, whose code 289e84b keeps: host and timing-sensitive x86 groups on un-rizen-service, the other x86 groups on i9, aarch64 split between them. "x86: boot, display, network…" and "x86: RAM above 4 GiB" timed out at 8 s while three tracks' gates loaded both machines (load average 32 and 27 on 16 threads) and passed when run again on the same tree.
- **For your branches.** Merge `main` and rebuild every image and program: ABI 11 refuses a bootloader or program of ABI 10. The kernel track's open tasks numbered on 2026-10-20 (351-KRN-0115 to 213-KRN-0123) plan ABI 12.
- **fast-test** is 3515368.
