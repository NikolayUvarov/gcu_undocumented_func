# 000-KRN-0134 — A program that asks for more memory than the loader has still starts

**Type:** `loader` · **Owner:** kernel session · **Priority:** P1 · **Status:** made, in `fast-test` (2026-10-20); the gate is left · **Blocked by:** — · **Main task:** — · **Constitution:** MC-3.13

## Problem

Gate g0132 on `main` abe3d70 (ABI 13), the "x86: keys, shell, tools" group, run on the kernel session's machine:
- **The failure.** `dictate --features fbank.wav` failed with `ERROR: TASK LIMIT REACHED (QUOTA)`.
- **The cause.**
  - `dictate` and `speak` ask for 384 MiB (`mind::request!(…, memory: 384)`).
  - `init`'s memory quota is the frame pool, and `loader` holds all of it (`SPAWN_MEMORY_ALL`).
  - SPAWN refuses a child's quota above the spawner's with `ERR_LIMIT`.
  - On QEMU's 512 MB machine the pool was 384 MiB under ABI 12 and is 383 MiB under ABI 13: the two new boot images cost it a MiB. So a request at the pool's size broke on a change elsewhere.
- **Not the design.** `loader` says a program's memory is "up to `APP_MEMORY_MAX` MiB; the frame pool limits it further". A quota is an upper bound, which the pool limits anyway: other tasks hold some of it.

The tools track recorded the same failure as a request on 2026-10-20, from its gate of 000-APP-0095 and 0096 on `main` abe3d70, and lowered both programs to 320 MiB (2a33a71). The loader's change still matters: any program can ask for more than a machine has.

## Plan

- **`loader` retries once.** When SPAWN answers `ERR_LIMIT` for a request above the default 16 MiB, `loader` starts the program with `SPAWN_MEMORY_ALL`: all of `loader`'s own quota, which is less than was asked.
  - It logs `[LOADER] <name>: <N> MiB ASKED, MORE THAN LOADER'S QUOTA: GIVEN ALL OF IT`.
  - SPAWN checks the quotas before it moves any grant, so the retry's grants are intact.
  - If the retry fails too, the first error is answered.
- **No ABI change.** SPAWN still refuses a child's quota above its spawner's (MC-3.13); only `loader` asks again.

## Acceptance criteria

- [ ] The tools suite's `dictate` checks pass on QEMU's 512 MB x86 machine with ABI 13's images.
- [ ] The whole gate passes on the tree with this change.

## Related

[800-KRN-0128](800-KRN-0128-init-starts-usb-serial-and-link.md) (the two boot images), issue 150 (memory quotas), `loader/src/main.rs` (`load`).
