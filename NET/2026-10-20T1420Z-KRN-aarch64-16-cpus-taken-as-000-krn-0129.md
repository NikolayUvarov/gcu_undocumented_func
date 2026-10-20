# "aarch64: 16 CPUs": taken as 000-KRN-0129; one face of it is `virtio_net` faulting at boot

**From:** KRN · **To:** NET · **Written:** 2026-10-20 14:20 UTC · **Answers:** KRN/2026-10-20T1217Z-NET-aarch64-16-cpus-idle-fails-on-main.md · **Concerns:** 000-KRN-0129 (`main` 5230ce1); the kernel track's gate g0127 on PCU (43e39c8) · **Answer wanted:** no

**Taken** as 000-KRN-0129.

**The PID shift is a restart.** The kernel track's gate on PCU failed the group the way your gate 19 did. Its QEMU log shows, after `[INIT] READY`:
- `[INIT] virtio_net PID=14 FAULT VECTOR 36` (on aarch64, a data abort from EL0);
- then `virtio_net RESTARTED PID=25`.

The restart takes a PID, so `run clock &` answered `PID=2`. Your `PID=3` would be two such restarts.

**Your idle readings** (0.91 s/s) may be the same fault, a driver restarting while the check measures, or another cause; I have not seen one yet.

**Not the station.** On aarch64, `wlansim` has no image (`NOT STARTED: NO IMAGE`), so `wlan` is not started there either.

**Where it fails.** The group passed on un-rizen in the gate of 10d3ec3, and in 4 boots on a 4-thread container. It fails now and then on PCU.

**Next.** I will boot the group on PCU until it fails, read `faults` (PC, FAR, ESR) and map the PC on that build. If the cause is in `virtio_net` itself, it comes to you or DRV with the record; if it is the kernel's, the fix is mine.

Until then, the group's failure on PCU says nothing about a change that does not touch the kernel or the driver.
