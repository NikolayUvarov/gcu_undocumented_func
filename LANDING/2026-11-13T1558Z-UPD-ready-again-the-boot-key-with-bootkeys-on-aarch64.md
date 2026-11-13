# UPD ready again: the boot key under the root and recovery orders, with the key file's refusals at boot on aarch64

**Ready:** `claude/351-UPD-0029-boot-keys` 20c5a354a1

**Replaces** UPD 1015Z (32d0486c), which round 129 neither landed nor returned (`LANDING/2026-11-13T1429Z`).

**Closes** 351-UPD-0029 and 351-UPD-0030 once it lands; 351-UPD-0031's document waits on it (text, `claude/351-UPD-0031-keys`).

**What is new since 1015Z:**
- **The `bootkeys` suite** (d161203f): the `boot` suite's signed-volume cases, a key file short of the root threshold among them, as a suite of its own, which `boot` still runs. "aarch64: the updater and the modes" runs it too (`scripts/ci_local.sh`, `.github/workflows/ci.yml`), so 351-UPD-0029's third criterion is shown on aarch64 as well.
- **Docs:** slots.md, slots_RU.md and the profile's Article 9 row name it.
- `main` d8c0ec82 merged in (text only).

**Tested:**
- **Round 129's gate** passed every group this branch touches at 32d0486c with the round's other requests: x86 boot (733 s), x86 updater (1676 s), aarch64 updater (1647 s), host tests (1834 s), and the rest.
- **At d161203f,** on x32-128 (QEMU under TCG):
  - "x86: boot, display, shell, memory, clock" (733 s);
  - "aarch64: the updater and the modes" (873 s), with `bootkeys`' line beside `update-bootkey`'s and `update-recovery`'s.
- **In this container:** `bootkeys` on x86 and on aarch64.
