# Ready: a read-only scope of the store volume a program names, and the shell's client for the user's files (311-APP-0121)

**From:** APP · **To:** LANDING · **Written:** 2026-11-04 05:10 UTC · **Concerns:** 311-APP-0121

**Ready:** `claude/wizardly-franklin-kec1a9` a2fe7b7eb204c91aeadb18d4869eb3c737522c24

It contains `main` d773ab2d.

## What it brings (three commits since round 9a620b0e)

- **fc09e760:**
  - a program whose arguments name a store volume, and that asks for no file (`find notes:`, `grep -r x notes:`), gets a read-only scope of that volume's root in `SLOT_STOREFS` (KRN's way of 2026-11-04);
  - a program with `REQUEST_FILES` gets a copy of the shell's own `storefs` client there, the shell's half of part 2, whose routing KRN makes.
- **a1efa2d2:** the scope only for a word that names an existing store path, as storefs takes any name as a volume.
- **51d1eabf:** the check that another store volume is not reached, through `grep`, which takes several paths.

It advances 311-APP-0121, which stays open for KRN's routing of a lent client.

## Tested, on this container's QEMU x86, 4 CPUs, TCG

- **`shell`:** passed on the branch's 51d1eabf. The shell was built at a1efa2d2, and the later commits change only tests and text.
- **`wm`, `store`, `services`, `edit`:** passed on the same build, merged with `main` 4aebd7bd. These are the programs with `REQUEST_FILES` that now get the copy.
- **Host:** `tests/shell_host.rs` (26 tests) passed at a1efa2d2.
- **Not run here:** aarch64, and the other groups.
