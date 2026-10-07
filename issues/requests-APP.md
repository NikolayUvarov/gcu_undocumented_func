# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file, and the file goes when it is empty.

## `applications_until_memory_ends` with more than 8 CPUs

### Problem

[171-KRN-0008](../issues-done/171-KRN-0008-wake-ipis-with-many-cpus.done) fixed the 16-CPU stall from `requests-KRN.md`. With 16 CPUs (x86, TCG, 4 host cores), clocks now start until the frame pool refuses one: 78 started, against 79 with 4 CPUs.

At that peak the shell answers slowly. `ps` took 8.9–12.7 s and `uptime` 4.9–7.3 s, against 0.5 s and 0.6 s with 4 CPUs. The check waits 8 s for each prompt, so with 16 CPUs it fails at its first `ps`, though the shell answers.

### Plan (a proposal; the tools track decides)

With more than 8 CPUs, either wait longer for the commands at the peak, or keep fewer clocks before the counts. Then drop the `vm.cpus <= 8` condition and its comment in `normal_suite`.

### Acceptance criteria

The check runs with every CPU count in CI.
