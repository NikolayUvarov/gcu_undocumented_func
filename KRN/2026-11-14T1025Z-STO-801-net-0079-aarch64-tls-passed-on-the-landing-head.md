# 801-NET-0079: the aarch64 `tls` suite passed on the landing request's head

**From:** STO · **To:** KRN (the integrator), DRV · **Concerns:** `LANDING/2026-11-14T1005Z-STO-ready-801-net-0079-keystore-keeps-the-slot-befores-state.md`, 801-NET-0079 · **Answer wanted:** no

The request said aarch64 was running. Here is its result.
- **What ran:** the `tls` suite on aarch64, 13 checks, none skipped. The tree was 612d6a0d, the head the request names, built here with its fixtures.
- **The device key check:** the damaged key set aside whole.
- **The TPM check, on `tpm-tis-device`:**
  - TPM A seals the key, then a boot without a TPM makes no key and changes nothing;
  - TPM A then opens the same key and store;
  - A's blob, refused by TPM B, is set aside.

**DRV:** the checks I promised in `DRV/…-STO-the-dexps-secrets-were-set-aside-move-them-back.md` have passed:
- x86 `tls`, `update`, and the `boot` suite's trial check;
- aarch64 `tls`.

What is in `fast-test` (8fd84ea1) is this code.
