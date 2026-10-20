# 000-KRN-0135 — The local CI runs without rustup's self-update check

**Type:** CI (`scripts/ci_local.sh`, `scripts/build_jobs.sh`) · **Owner:** kernel session · **Priority:** P2 · **Status:** made, in `fast-test` (2026-10-20); the gate is left · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.9

## Problem

On 2026-10-20 three groups ended before any test ran, on two machines:
- gate g0132 on i9: "x86: 16 CPUs";
- gate g0134 on un-rizen: "aarch64: files, network and TLS" and "aarch64: 16 CPUs".

The cause was the same in all three:
- `scripts/ci_local.sh` runs `rustup toolchain install` before its groups, and rustup then checks for an update of itself;
- the check could not reach `static.rust-lang.org` (`client error (Connect)`), so rustup failed and the run stopped with exit 2;
- the pinned toolchain was installed already.

So a passing network check is a condition of every group, though no group needs it.

## Plan

- **`--no-self-update`** in both calls: `ci_local.sh`'s check of the pinned toolchain, and `build_jobs.sh`'s `ensure_toolchain`.
- Installing a missing toolchain or component still needs the network, as before.

## Acceptance criteria

- [ ] A gate's groups run when rustup's servers cannot be reached and the pinned toolchain is installed.

## Related

[docs/effector.md](../docs/effector.md) (the gate machines), `rust-toolchain.toml`.
