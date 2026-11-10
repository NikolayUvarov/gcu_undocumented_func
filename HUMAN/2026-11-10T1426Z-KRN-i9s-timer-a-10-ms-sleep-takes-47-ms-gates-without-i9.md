# i9's timer: a 10 ms sleep in QEMU takes about 47 ms there today, so gates run without i9

**From:** KRN (the integrator, the kernel session) · **To:** HUMAN · **Concerns:** the landing gates, the i9 machine (`i9_4e6882212251`, WSL2) · **Answer wanted:** only if you know a cause, or want i9 back in the gates as it is

**What fails.** Today "x86: keys, shell, tools" failed on i9 three times: twice on round 112's candidate, the second time alone, and once on plain `main` 06f3a8b0.
- **The check:** `kbench`'s check that `sleep 10 ms` is not longer than 40 ms.
- **On i9:** median 47.0 to 47.7 ms, longest 66 to 68 ms.
- **What is still right there:** `sleep 1 ms` (median 1.85 to 1.92 ms).
- **Elsewhere:** the same group on the same candidate passed on acc-1 (477 s). On i9 it passed in rounds 102 to 110.

**What it looks like.** QEMU's timer on that machine is coarse today, about a host tick in a WSL2 guest. It is not the kernel: `main` fails the same way.

**What I do.** Round 113 ran its gate without i9, and passed. I leave i9 out of the gates until a run of the group on `main` passes there again. I check that before each round, and say so in `LANDING/`.

Nothing was changed on the machine.
