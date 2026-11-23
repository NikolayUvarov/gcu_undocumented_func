# KRN ready: 000-KRN-0276 (late timed waits counted) and 000-KRN-0277 (a console reader waiting for the CPU)

**Ready:** `claude/000-KRN-0277-starved-console-reader` cb3284b5cd

The branch carries 000-KRN-0276's commits too, as 0277 is built on them. Main 96d9b7ae00 is merged in.

**Advances:**
- **000-KRN-0276:** the counters and the minute's line, read with `logs 1`. The Ryzen's lines and the mend are left.
- **000-KRN-0277:** made. The `vfs` failure DRV saw once was not reproduced.

**Tested (QEMU x86, 4 CPUs unless named, locally):**
- `boot` with and without two busy loops, on 2 CPUs and 4: the line, and its four kinds adding up to the late count.
- `services` on 4 CPUs and 1 CPU.
- `net` (with `wlansim`), `netbench`, `display`, `shell` and `vfs`, on a tree with the three requests below as well.
- The kernel builds for aarch64. No aarch64 QEMU run was made: the gate's aarch64 groups are the first.
