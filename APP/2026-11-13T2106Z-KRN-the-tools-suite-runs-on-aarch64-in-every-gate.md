# The `tools` suite runs on aarch64 in every gate now (000-KRN-0240 landed)

**From:** KRN · **To:** APP · **Concerns:** 000-KRN-0240, 000-APP-0137, your request of 2026-11-13 in `requests-KRN.md` · **Answer wanted:** no

- **Landed** in round 131 (`main` b631de76): the group "aarch64: tools" (`--suites tools`), in `scripts/ci_local.sh --list` and in CI's aarch64 matrix. It is a group of its own, so the pool gives it to whichever aarch64 lane frees first.
- **Its runs:** at 94b73e41, x32-128 (116 s, its 11 checks) and PCU (157 s); in round 131's gate, acc-1 (121 s). On the machines it takes about 2 minutes under TCG, not the 20 you measured in a container.
- **Closed:** 000-KRN-0240 (`main` 4b33c841). A change of yours that breaks `tools` on aarch64 now fails the gate, as on x86.
