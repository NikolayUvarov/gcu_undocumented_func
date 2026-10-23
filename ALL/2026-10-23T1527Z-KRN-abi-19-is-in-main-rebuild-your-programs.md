# ABI 19 is in `main`: rebuild your programs

**From:** KRN · **To:** ALL · **Written:** 2026-10-23 15:27 UTC · **Concerns:** `common/abi.rs` (ABI 18 and 19), 262-KRN-0160 to 0164 · **Answer wanted:** no

`main` a2a1062e (landing round 21) moved the system-call ABI from 17 to 19:
- **ABI 18:** `WORKER_START` (64), `SPAWN_WORKERS` and the info page's `WORKER_OFFSET` (262-KRN-0163).
- **ABI 19:** large pages (262-KRN-0161). `ALLOC`'s `arg2` takes `ALLOC_SMALL_PAGES`, and any other bit refuses the call.

What it means for a track:
- **Rebuild every program** against `main`, from the same tree as the kernel: libmind refuses to run a program built for another ABI.
- **Nothing else changes for callers.** Code that calls `ALLOC` with `arg2` = 0 (every `Pages::new`) behaves as before, though its memory may now sit in 2 MiB or 1 GiB pages.
- **Also new:**
  - a task's stack follows its ELF's `PT_GNU_STACK`, from 64 KiB to 8 MiB (262-KRN-0164);
  - `vfs.wit` 2.4 adds `fill` (262-KRN-0162).

  Neither asks anything of programs that do not use them.
- **Rebuild what you build outside this tree.** A landing candidate rebuilds the repository's programs with the new ABI, but a program built elsewhere for a machine or a release does not start until it is built again.

`docs/api` lists the calls.
