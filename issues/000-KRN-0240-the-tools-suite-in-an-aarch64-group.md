# 000-KRN-0240 — The `tools` suite in an aarch64 group of the gate

**Type:** kernel track (the gate's groups) · **Owner:** `KRN` · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** — ([000-APP-0137](../issues-done/000-APP-0137-the-tools-suite-on-aarch64.done)) · **Roadmap:** track G · **Constitution:** MC-12.2

## Problem

APP's request of 2026-11-13 (from `requests-KRN.md`), for 000-APP-0137. The request as it was written, kept whole:

**Asked by** APP on 2026-11-13, for [000-APP-0137](../issues-done/000-APP-0137-the-tools-suite-on-aarch64.done).

- **What there is:** `scripts/ci_local.sh --list` runs the `tools` suite on x86 alone (`x86: keys, shell, tools`); no aarch64 group holds it. Its checks had drifted from what aarch64's `virt` shows (a 37-row screen, a VirtIO tablet, `BOOTAA64.EFI`, MPIDR affinities, the device key kept in `system/keystore/` where no TPM seals it), and nothing said so.
- **Done on the tools branch:** the suite passes on aarch64 (11 checks, 4 CPUs, in a container), `clipboard_check` among them, and still on x86 (11); `hw` names aarch64's CPU IDs and counter.
- **Asked:** `tools` in an aarch64 group, for instance `aarch64: programs, shell and four CPUs` (`--suites normal,shell,…`) or a group of its own, once the tools branch's request with it lands, so that it stays checked there. The group's time is yours to weigh: the suite took about 20 minutes on aarch64 under TCG here.

## Plan

A group of its own, "aarch64: tools" (`--suites tools`), rather than a longer "aarch64: programs, shell and four CPUs": the pool then gives it to whichever aarch64 lane frees first, and the 20 minutes do not lengthen the group that already takes 10.

## Acceptance criteria

- `scripts/ci_local.sh --list` and `.github/workflows/ci.yml` hold the group.
- The group passes on the maintainer's machines, then in a gate.

## Progress

**Made (2026-11-13)** on `claude/000-KRN-0240-tools-on-aarch64`: the group in `scripts/ci_local.sh` and in CI's aarch64 matrix.
- **Run** at 94b73e41 on the maintainer's machines (QEMU under TCG): "aarch64: tools" passed on x32-128 (116 s, its 11 checks, `clipboard`'s among them) and on PCU (157 s).
- **Left:** the gate.

## Related

[requests-KRN.md](requests-KRN.md), [000-APP-0137](../issues-done/000-APP-0137-the-tools-suite-on-aarch64.done).
