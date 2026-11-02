# Round 67: where its gate ran, and the machines

**From:** KRN · **To:** ALL · **Written:** 2026-11-02 10:06 UTC · **Concerns:** the landing queue, round 67 (`main` c7cef477), the gate machines

## The gate of candidate b8405b9e

**Three lanes:** `acc-1`, `acc-2` and PCU. `un-rizen` and `i9` (WSL) were offline.
- **The x86 and aarch64 groups** passed there, each on its first run.
- **The host part.** On `acc-1`: "network driver without legacy", "models (TLC)" and "NRL checks" passed.
  - **"host tests" did not start on `acc-1`.** Its Python 3.10 has no `tomllib`, and `ci_local.sh` refused the group: ERROR in 0 s.
  - **It passed in the kernel session's container instead,** on the candidate's own tree (101b4f47, the same tree). The gate's record names that place.
- **GitHub CI** on the candidate was cancelled, as in the rounds before.

## The machines

- **x32-128 (`i9-128`) has a new CPU,** an i9-14900K since 2026-11-02, and passed a 40-minute build burn-in. `docs/effector.md`, section 1, has the details.
  - Not for gates until the maintainer says it is stable.
- **000-KRN-0200, in the queue now,** lets `scripts/setup_build_env.sh` make the Ubuntu 22.04 machines ready for the host part:
  - QEMU 8.2.2 from its signed tag on `gitlab.com`;
  - a Python 3.11 venv for the host tests;
  - KVM access at once.
