# Requests for the porting track (not numbered)

**Owner:** porting track · **Status:** open · **Recorded by:** the tools track, 2026-10-06

The porting track numbers its own issues (200–249), so requests from other tracks wait here: the porting track turns each into an issue in its range and removes it from this file. The file goes when it is empty.

## aarch64: the idle check of the `normal` and `smp` suites has almost no margin

### Problem

On aarch64 the `normal` and `smp` suites check that the CPUs wait in WFI. The check: an idle QEMU (`virt`, 4 CPUs, TCG) must use less than 0.6 s of processor time in 1 s of wall time (`tests/qemu_smoke.py`, `qemu_cpu_seconds`).

Measured on 2026-10-06 on the tools branch (9ca3739: `virt,gic-version=3,highmem=off`, 4 CPUs, a 4-core host with nothing else running):

- Samples of 1 s at the prompt: 0.46, 0.46, 0.49, 0.56, 0.57, 0.58 and 0.58 s.
- One run of the `normal` suite failed with 0.61 s.
- Main's CI on 56fb7b2 failed the GICv2 job the same way, with 0.63 s.

What wakes at idle (`ps` twice, 5 s apart):

| Task | Wakeups per second | Why |
|---|---|---|
| `shell` | about 95 | `sleep(10)`: it polls the UART and voice, as it has since issue 036 |
| `netstack` | about 100 | |
| `compositor`, `virtio_input`, `virtio_net` | about 50 each | |

None of this is new. The limit sits inside the normal spread. So the check fails on a busy runner without a regression, and it would not see a real regression of a few percent.

### Plan (for the porting track to decide)

- **Measure the wait inside the guest.** `STAT` gives each CPU's `idle_ns` and `busy_ns` (`StatCpu`). A CPU that does not wait in WFI shows no idle time, whatever the host's TCG costs are.
- **Or take the lowest of a few samples**, as the x86 branch does with `HLT=1`.
- **Separately, fewer wakeups at idle.** For example, the shell could read the UART on its interrupt, or sleep longer after a quiet second. This is the tools and kernel tracks' work, on request.

### Acceptance criteria

The aarch64 idle check passes on a busy runner (for example, with a parallel build at `nice 19`). It still fails when a CPU spins instead of waiting in WFI; the porting track's test for that stays.

### Related

[201](../issues-done/201-aarch64-boot.done), [203](../issues-done/203-aarch64-smp-and-power.done), [204](../issues-done/204-aarch64-profile-and-ci.done).
