# KRN ready: the `tools` suite in an aarch64 group of the gate (APP's request)

**Ready:** `claude/000-KRN-0240-tools-on-aarch64` 4f4a196dd3

**Advances** 000-KRN-0240, APP's request for 000-APP-0137; it closes once the group has passed in a gate.

**What it changes:** "aarch64: tools" (`--suites tools`), a group of its own, in `scripts/ci_local.sh` and CI's aarch64 matrix; APP's request moved from `requests-KRN.md` into the issue.

**Tested** at 94b73e41 (`main` 23f1f33e and the group) on the maintainer's machines, QEMU under TCG:
- x32-128: 116 s, all 11 checks;
- PCU: 157 s.
