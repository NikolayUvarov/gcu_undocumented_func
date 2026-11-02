# 000-KRN-0200 — `setup_build_env.sh` for the Ubuntu 22.04 gate machines: QEMU from gitlab.com, Python 3.11, KVM at once

**Type:** scripts · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made; waits for its landing and the maintainer's run on x32-128) · **Blocked by:** — · **Roadmap:** the landing queue's machines (AGENTS.md, section 4) · **Constitution:** MC-12.1

## Problem

x32-128, after its CPU swap (2026-11-02, [docs/effector.md](../docs/effector.md), section 1):
- **QEMU.** `scripts/setup_build_env.sh` fetches QEMU 8.2.2's tarball from `download.qemu.org`, which times out from that machine, while `gitlab.com` answers. Under `set -e` the script stopped there, before its KVM step.
- **KVM.** So `gater` is not in `kvm` and cannot open `/dev/kvm`.
- **Python.** Ubuntu 22.04's Python is 3.10, without `tomllib`, so the host tests cannot run there (nor on `acc-1` and `acc-2`).

## Plan

1. **QEMU 8.2.2 from its tag `v8.2.2` on `gitlab.com`:**
   - the tag's signature is checked against Michael Tokarev's primary key `6EE195D1886E8FFB810D4324457CE0A0804465C5`, who signs QEMU's stable tags;
   - its commit is checked against the pinned `11aa0b1ff115b86160c4d37e7c37e6a6b13b77ea`;
   - meson fetches the subprojects from `gitlab.com` at the revisions the signed tree's `.wrap` files pin.
2. **Python 3.11:**
   - where `python3` is older, Ubuntu's `python3.11` (22.04: `3.11.0~rc1-1~22.04.1`, jammy-updates/universe) goes into a venv, `/opt/mind-python3.11`;
   - the venv leaves out the system's packages, whose `cffi` is built for 3.10 only;
   - `scripts/ci_local.sh` puts it first in `PATH` for the host tests alone. The Secure Boot groups keep the system's `python3-virt-firmware`.
3. **KVM:**
   - it is set up before the QEMU build, so that a failed fetch no longer leaves it out;
   - `usermod -aG kvm` takes effect for the agent's service at its next start, and an ACL on `/dev/kvm` at once.
4. **`--only PART[,PART...]`** runs single parts: `packages`, `kvm`, `python`, `rust`, `qemu`.

## Acceptance criteria

- On x32-128, `sudo scripts/setup_build_env.sh gater` ends with `Ready`:
  - `gater` opens `/dev/kvm`;
  - `qemu-system-x86_64 --version` says 8.2.2;
  - `scripts/ci_local.sh --only host --group "host tests"` passes there.

## Progress (2026-11-02)

- **Made:** `scripts/setup_build_env.sh`, `scripts/ci_local.sh`, and the guide's paragraph on setting up a machine.
- **Tested on x32-128, as `gater`:**
  - the tag check as the script runs it: signature OK, commit OK, a check against another signer's key refused;
  - apt's candidates for `python3.11` and `python3.11-venv`.
- **Tested here:** `bash -n` of both scripts.
- **Not run yet:** the root parts. They are the maintainer's to run.

## Related

[docs/effector.md](../docs/effector.md), sections 1 and 6.
