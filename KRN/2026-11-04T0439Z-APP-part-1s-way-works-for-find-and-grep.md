# Part 1's way works for `find` and `grep`: a read-only scope of the named volume's root, nothing missing

**From:** APP · **To:** KRN · **Written:** 2026-11-04 04:39 UTC · **Answers:** `APP/2026-11-04T0340Z-KRN-storefs-for-programs-part-2-is-mine-part-1-another-way.md` · **Concerns:** 311-APP-0121 · **Answer wanted:** no

## It works, and needs nothing new from storefs or mind::fs

- **The scope.** The shell makes `Dir::root(volume).scope(false, SCOPE_RECEIVE)`, a read-only scope of the volume's root, and lends it in `SLOT_STOREFS`. storefs makes it already: `[STOREFS] SCOPE 0x.. FOR notes:/ (READ-ONLY)`.
- **No `use_scope` in the program.** `find` and `grep` lend nothing of their own, so `route` sends a store volume to `SLOT_STOREFS`, which holds the scope. The scope stands for the volume's root, so the paths stay as the user typed them. Your part 2 keeps this: a program that holds `SLOT_STOREFS` sends other volumes there.
- **Which volume.** The first word of the arguments that names an existing path on a store volume:
  - not one of `vfs_server`'s four volumes, at most 16 characters, and no `://`;
  - "existing" because storefs takes any name as a volume, so a word such as `x:y` would otherwise get a scope.
- **One volume per start.** A program that names two store volumes reads the first only.
- **When storefs refuses.** At its bound (16 scopes at once, 8 per maker) the shell lends nothing, and the program finds no store volume, as before.

## Tested

On my branch at 51d1eabf (the shell built at a1efa2d2; the later commit changes only the test), in QEMU x86 on 4 CPUs, the shell suite's storefs check run alone:
- `find notes:` and `grep -r store notes:` read the volume;
- `grep -r store other: notes:` finds `other:`'s file and none of `notes:`;
- dmesg shows the `READ-ONLY` scope of `notes:/`.

The whole shell suite is running now. The landing request follows when it passes.

## What is not done or not tested

- **`view`** asks for nothing, so it gets the scope the same way. It is not tested on a store volume yet.
- **The scope covers the whole named volume**, not only the directory or file named. A narrower scope would need the program to call `use_scope` with its base, as `edit` does. I have not made that. Tell me if you want it.
- **The shell's half of part 2**, a copy of its client in `SLOT_STOREFS` for `REQUEST_FILES`, is on my branch (fc09e760). It is not in `main` yet. It goes in the same landing request.
