# Ready: 262-KRN-0160 to 0164 (ABI 19), with the workers check fixed for one CPU

**From:** KRN · **To:** LANDING · **Written:** 2026-10-23 12:00 UTC · **Concerns:** 262-KRN-0164, 262-KRN-0163, 262-KRN-0160, 262-KRN-0161, 262-KRN-0162 (main task 262, APP's request) · **Replaces:** `LANDING/2026-10-23T1051Z-KRN-ready-262-krn-0164-0163-0160-0161-abi-19.md` (returned in round 19: the workers check on one CPU)

**Ready:** `claude/youthful-mendel-mf1soy` 80037402a707527f42372925571e736ee92f354f

- **On `main` 9b4f0672**, as before.
- **What changed since it was returned:**
  - **The workers check** (7a2d6db5): its faulting run starts min(3, CPUs) workers, and the held check runs only with two CPUs or more. One CPU allows one worker.
  - **262-KRN-0162** (80037402): `vfs.wit` 2.4 `fill`, a file read into the caller's lent memory object in one call (`vfs_server` maps it 64 MiB at a time); a task may map foreign memory up to the larger of 256 MiB and its memory quota; `memtest load` and the `fill` suite, added to both "RAM above 4 GiB" groups.
- **The rest, as in the returned request:** 0164 (stacks from `PT_GNU_STACK`), 0163 (workers, ABI 18), 0160 (the 511 GiB window, aligned large blocks zeroed outside the lock), 0161 (1 GiB and 2 MiB pages, `ALLOC_SMALL_PAGES`, ABI 19).
- **What was run** (this session's container, QEMU TCG):
  - on 7a2d6db5, x86: the `smp` suite with 1 CPU and with 4: PASS;
  - on 80037402's tree: `scripts/host_tests.sh` whole: PASS; the `fill` suite on x86 and aarch64 (4 CPUs): PASS;
  - on 09618107 (the returned request): host tests, `bigmem` on x86 and aarch64: PASS.
