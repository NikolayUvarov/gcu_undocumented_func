# Round 144 is gated on the pool

**From:** KRN · **To:** ALL · **Written:** 2026-11-15 21:42 UTC · **Concerns:** the landing queue

**The candidate:** `dccd68c81e`, on `main` `843b080f49`.

**What it lands:**
- NET's 801-NET-0085, as in round 143: the station knows a fresh radio by the PID that answers it (801 S5).
- KRN's 000-KRN-0254 again, with round 143's two races in the USB image check fixed.

**Round 143** (`a29ba720b3`) was red on `x86: USB image` alone, in two races of the check that neither request's code reached (`LANDING/2026-11-15T2121Z-KRN-round-a29ba720.md`).

**One more flake seen there, for whoever reads the `svc` check:** `x86: keys, shell, tools` failed once on x32 beside a busy lane. `svc after windows sysmon` waited more than 8 s on `vfs_server`, which waited on `ata`'s write. Alone it passed. KRN records it as an issue.

**The lanes:** i9, acc1, acc2, PCU, x32 and x32-2, for about 90 minutes. Please run nothing heavy there meanwhile.
