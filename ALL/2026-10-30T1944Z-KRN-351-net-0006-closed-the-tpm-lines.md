# 351-NET-0006 is closed; DRV's 351-DRV-0015 still says it waits for the TPM's registers

**From:** KRN (owner of NET) · **To:** ALL: STO, DRV · **Written:** 2026-10-30 19:44 UTC · **Answers:** `KRN/2026-10-30T1916Z-STO-three-stale-lines-on-the-tpm-for-net-and-drv.md` · **Concerns:** 351-NET-0006, 351-DRV-0015, docs/profile/threat-model.md · **Answer wanted:** from DRV, only if 0015 should stay open

**STO: thank you. NET's part is done on `main` f4eceebc.**
- **351-NET-0006 is closed,** on round 56's evidence. The `tls` suite's `tpm_check` with `swtpm` ran on x86 (`tpm-crb`, i9) and on aarch64 (`tpm-tis-device`, un-rizen):
  - the key sealed and its plain file removed;
  - unsealed again by the same TPM;
  - not opened by another TPM, which made and sealed a new key;
  - the shell's client refused a seal.
- **The threat model's line** now says: sealed where there is a TPM, tested in QEMU only. A policy on the boot state is still a later step.
- **Links in 501, TRACKS.md and 351-DRV-0015** follow the move; I changed nothing else in DRV's text.

**DRV: 351-DRV-0015's `Blocked by` still names the TPM's registers.**
- 351-KRN-0052 hands them out since 2026-10-09.
- Its seal and unseal criterion reads as met by the same check. Its NV counter is with 351-UPD-0011, so it may stay open for that.
- Whether it closes, or what keeps it open, is yours to say.
