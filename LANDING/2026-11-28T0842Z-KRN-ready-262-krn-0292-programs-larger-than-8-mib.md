# KRN ready: 262-KRN-0292, programs larger than 8 MiB, IMAGE_MAX 512 MiB (ABI 40)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-28 08:42 UTC

**Ready:** `claude/262-KRN-0292-programs-larger-than-8-mib` 44ac99c7f7

**Advances:** 262-KRN-0292, step 1. Step 2 (no contiguous run, shared read-only segments, `PT_TLS`) waits in `requests-KRN.md` for 264 or a native inference runtime.

**Carries** 100003-KRN-0291's commit (cd0e0fd0e3): this branch is built on it. Land it after that request; if that request is returned, this one is returned with it.

**What it is.** APP's request of 2026-11-27, at the maintainer's word.
- **ABI 40:** `IMAGE_MAX` 512 MiB in `common/abi.rs`, one bound for every program; its memory quota bounds what it really takes.
- **The kernel** takes an image whose loaded segments span up to `IMAGE_MAX`; past it, `ELF IMAGE ABOVE 512 MIB` and `ERR_INVALID`. **The loader** reads a program file up to `IMAGE_MAX` (4 MiB before).
- **The windows** (stack, screen, information page, mailbox, exit page, heap) lie above the image window, 496 MiB higher than before. An unmapped page stays below the stack.
- **Left as it was, and stated in the issue:** one contiguous run of the frame pool for the image, no sharing between instances, 64 section headers read for the request note.

**Tested on its own:** `tests/runtime.rs` pins (48, 40) and loads `app.elf` grown to just under 512 MiB; the `memory` suite's `large_image_check` on x86 ("PASS: a program spanning 72 MiB from a 6 MiB file ran; one past IMAGE_MAX (512 MiB) was refused, nothing kept"); every group below with the windows moved.

**Tested together.** The kernel session's six requests for round 183 were merged with `main` 4df3fe9842 into `claude/KRN-precheck-183`. `main`'s commits since then are text only, merged into each branch afterwards.
- **Host tests here:** 5931 passed, 0 failed (every `tests/*_host.rs` the quick runner builds, and `runtime.rs`); `tests/release_test.py` 19, one skipped as before.
- **On the machines,** each group once. Each later tree adds only the test fixes below.
  - 9b87ca5c92: aarch64 "network, TLS and Effector" (acc1, 1596 s) and "boot and fault containment" (acc2, 177 s); x86 "the updater and an update through the link" (x32-2, 1316 s).
  - 945df29ff2: x86 "boot, display, shell, memory, clock" (x32, 1583 s), "the window manager" (700 s) and "keys, shell, tools" (559 s); aarch64 "programs, shell and four CPUs" (acc2, 472 s) and "tools" (157 s).
  - e730dd468c: x86 "network, TLS and Effector" (acc1, 2070 s).
  - 8e477e9288: x86 "services, storage, audio" (x32, 1172 s) and "scheduling, isolation, heap" (x32-2, 151 s).
  - The gate runs every other group.
- **Four misses of mine, all in test expectations, all fixed on the branches:**
  - the stack check and two other QEMU checks still named the windows' addresses from before 262-KRN-0292 (d4747deb82; it failed aarch64's programs group);
  - the services suite's pmap check, the same (ddf4a12745; it failed x86's services group);
  - the isolation fixture's probes of the guard, the mailbox's edge and the stack, the same (1f5b9f54fd; found by reading, before a run);
  - APP's hub check asserted parse's refusal of a long text "until NET's change", which 100003-NET-0100 is (979ba00e45; it failed x86's network group; APP is told).
- **In `fast-test`** (a9ba59fa6d), all six.
- QEMU's runs say nothing of a real machine (MC-12.1).
