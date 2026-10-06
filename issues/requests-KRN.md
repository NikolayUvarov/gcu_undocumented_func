# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open · **Recorded by:** the tools track, 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here: the kernel track turns each into an issue and removes it from this file. The file goes when it is empty.

## The rest of issue 171

### Problem

Issue 171 (limits from the hardware) is a kernel main task. Step 1, all RAM on x86-64, is 171-KRN-0001. The tools session did step 2 at the user's request before TRACKS.md gave the kernel files to the kernel track alone: no task or endpoint limit but memory (`171-KRN-0002` in 171's table). Steps 3–6 are left: CPUs from the firmware (`cpu::MAX`), capability spaces that grow, the frame pool's ranges, and a program's memory (the heap window, `APP_MEMORY_MAX`, the block count, the global caps).

### Plan

See [171](171-limits-from-the-hardware.md), "Plan" and "Progress". The monitors' interface lists 8 CPUs (`sysinfo.wit` `cpus`, `sample.busy-low/high`): the tools track changes it once the kernel reports more.

### Acceptance criteria

Those of 171.

## The busy suite's share check on one CPU depends on the host

### Problem

`busy_suite` checks that without a budget the busy loop gets more than 0.6 of its one CPU, measured against the host's wall clock over 3 s. Under TCG a host that deschedules the emulated CPU takes that time from the loop. In a full local run (`--cpu-model max --cpus 1`, nothing else running in the session) it measured 0.52 once and failed; a rerun passed. Eight runs each on `main` (ed08bed) and on the tools branch (909be4f) gave the same spread: 0.62–0.73, mean 0.69. So the margin above 0.6 is small and host noise can cross it.

### Plan

The kernel track decides: measure the loop's share against the guest's own time (the CPU's busy plus idle time from `STAT_CPUS` over the same interval) rather than the host's wall clock, or another way that keeps the check as strict.

### Acceptance criteria

The check keeps its meaning (no budget: the loop takes most of its CPU) and does not fail when the host is busy.
