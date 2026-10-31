# 000-KRN-0197 — The aarch64 idle check measures QEMU's busiest thread, not the host's speed

**Type:** tests · **Owner:** `KRN` · **Priority:** P1 · **Status:** in progress (made 2026-10-31; runs on PCU left) · **Blocked by:** — · **Main task:** — · **Roadmap:** the landing queue (AGENTS.md section 4) · **Constitution:** MC-12.1, MC-12.2

## Problem

- **What happens.** On aarch64 the `normal` and `smp` suites check that idle CPUs wait in WFI. QEMU's monitor shows no WFI state there, so since issue 208 the check compares QEMU's processor time over a second with 0.85 s.
- **Where it fails.** On PCU (4 x86-64 cores, TCG), with nothing wrong in the guest:
  - round 45, "aarch64: 16 CPUs": 1.05 s;
  - round 60, "aarch64: GICv2 with GICv2m": 0.87 s;
  - round 62, the same group: 0.85 s.

  Each run passed alone on `un-rizen`. Earlier, 000-NET-0036's runs on PCU failed it at 0.86 to 0.96 s, which keeps 000-KRN-0129's last criterion ("aarch64: 16 CPUs" 5 times in a row on PCU) from being met.
- **Why.** An idle guest's processor time grows with the host's slowness and with the guest's CPU count. Issue 208 measured 0.46–0.58 s on a faster host; on PCU an idle guest is near the limit by itself. The figure says how fast the host emulates the guest's wakeups, not whether a CPU spins.

## Plan

- **What a spinning CPU looks like on any host.** Under TCG a vCPU that spins instead of waiting in WFI keeps its thread running or waiting to run for the whole second. That holds on a fast host, a slow one and a loaded one. An idle vCPU's thread sleeps between wakeups.
- **The check:**
  - from `/proc/<qemu>/task/*/schedstat`, the share of a second that the busiest QEMU thread ran or waited to run;
  - the lowest of three 1 s samples, against 0.9: a thread that never sleeps is runnable all of it;
  - QEMU's processor time in all is kept for the failure's message only.
- **Its figure in the suites' lines,** so that each run records what it measured on its host: `WFI (a thread runnable N%)`.
- **PCU takes "aarch64: 16 CPUs" again.** `scripts/land.py` kept it off PCU (`NOT_ON`, round 45) only because of this check. The rule and its one entry go.

## Acceptance criteria

- [x] With a kernel whose idle CPUs spin (`nop` for `wfi` in `wait_for_interrupt`), the check fails (a mutation run, recorded below).
- [x] "aarch64: GICv2 with GICv2m" and "aarch64: 16 CPUs" pass on PCU, with their figures recorded, and on `un-rizen` (below).
- [ ] The landing gate runs them with no idle-check failure.

## Progress

- **2026-10-31, made** on `claude/youthful-mendel-mf1soy` (f4c42ea3 at 0.6; then 0.9).
- **Mutation, local** (a 4-thread container, QEMU TCG, the `normal` suite on aarch64):
  - with `nop` for `wfi`, the check failed: "a thread ran or waited to run 100% of a second (3.96 s of processor time in all)";
  - the same tree without the mutation passed at 15%.
- **PCU, f4c42ea3:** "aarch64: GICv2 with GICv2m" passed in 880 s. The busiest thread was runnable 47% in the `normal` suite and 45% in the `smp` suite.

  These figures led to the limit of 0.9 instead of 0.6: an idle thread there waits for the host's 4 cores, and a spinning one is runnable all of the second whatever the host.
- **PCU, cf0161af (the limit 0.9, `NOT_ON` gone):** both groups passed:
  - GICv2 in 884 s, at 46% and 45%;
  - "16 CPUs" in 301 s, at 41%. PCU had not run it since round 45.
- **PCU, cf0161af, four more runs of the 16-CPU group's suite** (`--cpus 16 --suites normal`) on the same tree: each passed, in 296 to 301 s, at 40%, 41%, 42% and 41%. That makes five in a row with the run above, which 000-KRN-0129 and 000-NET-0036 asked for.
- **`un-rizen-service`, f4c42ea3:** both groups passed: GICv2 727 s, at 14% and 15%; "16 CPUs" 429 s, at 37%. The 16-CPU run was under a load average of 58 on 16 cores (two KRN lanes and another track's run).

## Related

- 208 (done): the check's margin, the lowest of three samples.
- 000-KRN-0129: "aarch64: 16 CPUs" 5 times in a row on PCU.
- 000-NET-0036: its last criterion waits for this check.
- Rounds 45, 60 and 62 of the landing queue.
