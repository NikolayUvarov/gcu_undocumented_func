# `ci_local.sh --only host` now asks for sshd: i9 and i7-930 have none

**From:** KRN · **To:** every track that gates through Effector · **Written:** 2026-10-14 15:08 UTC · **Answers:** — · **Concerns:** `scripts/ci_local.sh`:128 (351-NET-0026, `main` fef5f29) · **Answer wanted:** no

## What happens

- Since fef5f29, `scripts/ci_local.sh` stops at once with `Missing: sshd (openssh-server)` when `--only` names `host` and `/usr/sbin/sshd` is absent. The SSH client's host tests start their own sshd.
- The kernel track's gate of b8a7b3d passed `--only host,x86` with every group. On i9 and i7-930 each group then ended in 0 s, with no `PASS` or `FAIL` line.

## Where sshd is (checked read-only, 2026-10-14 15:00 UTC)

| Machine | `/usr/sbin/sshd` |
|---|---|
| un-rizen-service | present |
| PCU | present |
| i9 | absent |
| DESKTOP-i7-930-wsl-u24 | absent |

## What works meanwhile

- Pass each group its own part: `--only x86` for `build (x86 test programs)` and `x86: …`, and `--only aarch64` for `build (aarch64)` and `aarch64: …`. Neither asks for sshd.
- Run the `host` groups (`build (x86)`, the network driver, `host tests`, `models (TLC)`) on un-rizen-service or PCU.
- An x86 QEMU group needs `02_build.sh`'s output first. On a machine without sshd, run `./02_build.sh` itself rather than the `build (x86)` group.

## Asked of the maintainer (in chat)

Run `scripts/setup_build_env.sh` as root on i9 and i7-930. It now installs `openssh-server`.
