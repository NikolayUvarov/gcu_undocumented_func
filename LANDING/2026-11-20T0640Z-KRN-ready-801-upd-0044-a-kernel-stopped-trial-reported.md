# KRN: ready — a trial the kernel stopped is reported with its fault's line (801-UPD-0044, 801-KRN-0265)

**Ready:** `claude/801-UPD-0044-kernel-fault-reported` 74a3a1734e

**What it lands:**
- 801-KRN-0265 (advances):
  - the kernel's fault record's layout in `common/abi.rs`;
  - the kernel names the record and leaves it; `init` reads and deletes it and answers it as `init.wit` 1.14 `last-fault` (a function added at the end, a minor version);
  - a trial the kernel restarts at its deadline is recorded too.
- 801-UPD-0044 (advances): the updater of the slot a trial fell back to asks `last-fault` before it serves any request, and keeps a record of that trial's run in `system/updater/refused`, in the form `init`'s refusals have. The agent then reports it (801-EFF-0029), with no change on EFF's side.
- The x86 `rollback` suite gains release 7, which carries the `panic-test` kernel.

**Tested on the branch** through Effector (QEMU only):

| Group | Machine | Result |
|---|---|---|
| x86: a release that loses the server, rolled back (with the new check) | i9 | PASS, 854 s |
| x86: boot, display, shell, memory, clock | i9, then x32 after main was merged | PASS, PASS |
| x86: the updater and an update through the link | x32 | PASS, 1377 s |
| x86: services, storage, audio | i9 | PASS, 1202 s |
| aarch64: boot and fault containment | acc-1 | PASS |
| aarch64: the updater and the modes | acc-1 | PASS |
| aarch64: a release that loses the server, rolled back | acc-1 | PASS |
| host: `idl_fuzz`, `updater` | here | PASS |

The x86 updater group failed once on i9: the shell stopped answering 63 s into a boot that does not take the new path. It passed on x32. The issue records the details.

The kernel is at 7977 lines of 8000 (7981 with 213-KRN-0125's request).
