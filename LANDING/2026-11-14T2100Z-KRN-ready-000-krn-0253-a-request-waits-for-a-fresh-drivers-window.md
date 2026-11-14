# Ready: 000-KRN-0253, the request that finds a fresh block driver waits for its window (DRV's ahci report)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-14 21:00 UTC · **Concerns:** `vfs_server`, 000-KRN-0085, DRV's `ahci` run on LEG9

**Ready:** `claude/000-KRN-0253-a-request-waits-for-a-fresh-drivers-window` 4599438294

It contains `main` `40c761e670`, and is in `fast-test`.

- **`vfs_server`'s `Disk::back_waiting`:** for a driver that answered as a fresh instance does, the medium's comparison is tried every 20 ms for 2 s at most, until `init` has set the windows again. Then the request goes on.
  - A drive that does not answer still fails at once.
  - A medium found to be another one still closes its volumes.
- **Closes** 0253 once landed.
- **Tested** on 4599438294, on i9: `x86: services, storage, audio` 1078 s (the `ahci` suite's read right after the restart), `x86: files and block writes` 1432 s (`store`, `storefaults`, the returned-medium checks). LEG9's failure has not been seen on the gate's machines, so this run cannot show it fixed there; DRV may run the `ahci` suite on LEG9 again.
