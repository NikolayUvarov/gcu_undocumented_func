# 000-KRN-0105 — README, AGENTS.md, CONTRIBUTING.md and the profile agree with the code

**Type:** documents (text only) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.3, MC-12.4

## Problem

From the critique of 2026-10-14 ([issues-critica-done/01](../issues-critica-done/01_constitution_and_docs_analysis.md), §1.2 and §2), checked against `main` dfc42ee. Line numbers are that commit's.

- **README's table of system calls** (README:638 onwards) against `common/abi.rs`:
  - `STAT` is listed twice. Row 674 (`51`, class and buffer) is right. Row 681 (`48`, with another order of arguments) is wrong: 48 is `TASK_WATCH`.
  - `MEM_MAP` (16) has two rows, 667 and 678, which describe one call used two ways.
  - Calls 54–57 and 59–61 are missing (`DEVICE_CONFIG`, `REBOOT`, `INPUT_POINTER`, `INPUT_LISTEN`, `MEMORY_RESERVE`, `BOOT_CONFIRM`, `FIRMWARE_VARIABLE`). [docs/api](../docs/api/README.md) lists them.
- **README's paragraphs that no longer hold:**
  - README:213: "NVMe is not supported yet". README:92 and CI boot from NVMe on both architectures.
  - README:217: `audio_gw` described with AC97 only. It drives Intel HD Audio as well (551-DRV-0010).
  - README:70: "x2APIC is not supported". README:554: "with xAPIC". x2APIC mode is used (211-PRT-0002) and CI tests it.
  - README:357–358 asks to disable Secure Boot. That still holds for a default build, which is unsigned, but not for one signed with `MIND_SECURE_BOOT_KEYS` (351-UPD-0012).
  - README:542–548: the session example starts programs at PID 1–3, while README:115 says applications start at PID 19.
  - README:68 and docs/profile/kernel-objects.md:7: "96 capability slots" at a task's start. `CAP_SLOTS` is 104.
  - README:76: budgets "enforced at the 10 ms tick". Since 000-KRN-0083 the tick is 1 ms on x86, and budgets are enforced at the end of each 10 ms slice.
- **Versions named in other documents:**
  - README:53 calls the roadmap "v1.2";
  - constitution/README.md:9 calls it "1.1 (2026-10-03)";
  - the roadmap itself is 1.9. 000-KRN-0106 fixes the roadmap's own heading.
- **AGENTS.md:38** lists twelve track codes. It leaves out `EFF` (open) and `DEV` (proposed), which [TRACKS.md](../TRACKS.md) registers.
- **CONTRIBUTING.md:3** names x86-64 only. aarch64 has its own profile ([docs/profile/aarch64](../docs/profile/aarch64/)).
- **The profile's memory model.** Appendix B.1 of the Constitution names Untyped pools and verifiable retype as the initial model, and allows another "provided the profile is explicitly revised" (Constitution v1.6, line 354). The kernel keeps a frame pool with per-task memory quotas (`SYSCALL_ALLOC`, `SYSCALL_FREE`, 16 MiB by default). docs/profile never says it departs from B.1.
- **Revocation's cost** (MC-5.4) is not stated in the profile (000-KRN-0104).

## Plan

1. **README:**
   - delete row 681 and merge the two `MEM_MAP` rows;
   - make [docs/api](../docs/api/README.md) the full list: README keeps a short table and says so, so that the two do not drift apart again;
   - correct NVMe, audio, x2APIC, the slot count, the budgets' slice and the session's PIDs;
   - make the Secure Boot sentence conditional, with a link to [docs/update/secure-boot.md](../docs/update/secure-boot.md);
   - name the roadmap without a version number.
2. **constitution/README.md:** the roadmap's current version, or no number.
3. **AGENTS.md:** refer to TRACKS.md for the codes instead of repeating them, or add `EFF` and `DEV` (proposed).
4. **CONTRIBUTING.md:** x86-64 and aarch64, each with its profile.
5. **docs/profile:**
   - the MC-1.7 row and kernel-objects.md say that the kernel keeps a frame pool with hierarchical quotas in place of B.1's Untyped and retype, and why. This is the explicit revision B.1 asks for;
   - the MC-3.4–3.6 row says how revocation's cost grows and that MC-5.4 is not met for it until 000-KRN-0104;
   - kernel-objects.md: 104 slots.

## Acceptance criteria

- [ ] Every row of README's table of system calls matches `common/abi.rs`, and README names docs/api as the full list.
- [ ] None of the stale statements above remains in README, and `grep -n "96 capability\|not supported yet\|x2APIC is not" README.md docs/profile` finds nothing.
- [ ] AGENTS.md and CONTRIBUTING.md agree with TRACKS.md and the profiles.
- [ ] docs/profile states the memory model's departure from B.1 and the open MC-5.4 item for revocation.

## Related

000-KRN-0104, 000-KRN-0106, [issues-human](../issues-human/README.md) section 9 (the Constitution's and RFC's own changes).
