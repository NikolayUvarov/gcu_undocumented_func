# KRN ready: the long CI groups split, and the landing gate's pool

**Ready:** `claude/youthful-mendel-mf1soy` 42866b51

`main` f685cefe is merged in. This replaces the request of 15:18 (1e2f74ad): its first gate, on candidate 05d68f96, failed the host tests on this branch's own new check, which assumed an order of the lanes' threads (fixed in 42866b51), and ran three lanes on un-rizen, which slowed its timing checks (two lanes now, 10e1f7bb). The maintainer asked for the split and the pool on 2026-10-29.

## What it lands

- **ab373580, f3f7c80d: the long groups split.** No group runs much past 15 minutes now. They are the same suites in more groups, in `ci.yml` and `scripts/ci_local.sh` alike; docs/effector.md, section 4, has each suite's time.

  | Group before | The groups now |
  |---|---|
  | x86: boot, display, network, TLS, shell, memory, clock | x86: boot, display, shell, memory, clock; x86: network, TLS and Effector |
  | x86: keys, shell, tools | x86: keys, shell, tools; x86: the window manager |
  | x86: files and block writes | x86: files and block writes; x86: the updater and an update through the link |
  | aarch64: files, network and TLS | aarch64: files and the block store; aarch64: network, TLS and Effector; aarch64: the updater and the modes; aarch64: an update through the link |
  | aarch64: RAM, ACPI and PCI above 4 GiB | aarch64: RAM, ACPI and PCI above 4 GiB; aarch64: network above 4 GiB |

  **If your track's gate or report names a group,** the new names apply after this lands.
- **`scripts/ci_local.sh --group =NAME`** runs that group alone. `--group TEXT` still matches any group whose name holds TEXT.
- **`scripts/land.py --preset pool` (and `pool-no-i9`).**
  - Each lane takes the longest group left that it can run as it frees, and builds a part's image the first time it needs it.
  - The lanes: i9, `acc-1`, `acc-2`, two clones on `un-rizen` and PCU.
  - A gate started again gives each lane the same groups first.

## Tested

- **Each suite of the split groups, alone** on `3e07edb8` (un-rizen, acc-1, acc-2, PCU, through Effector): every one passed.
- **`tests/land_test.py`:** all passed.
- **`ci_local.sh --list`:** the groups as above.
- **This round's own gate** runs on the candidate with `--preset pool`, so it gates the split groups on the machines too.
