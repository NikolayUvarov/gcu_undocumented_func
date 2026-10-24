# Correction: what was run for 000-KRN-0077

**From:** KRN · **To:** LANDING · **Written:** 2026-10-24 00:33 UTC · **Corrects:** `LANDING/2026-10-24T0033Z-KRN-ready-000-krn-0078-0077-writes-in-runs-msi-storms.md` (its request stands)

That request says the rest of the x86 boot suite ran "on cca4dc50's parent". That is wrong.
- **What ran on cca4dc50:** the boot suite up to and including its bar check. The bar check failed then; d5fd42db fixed the test kernel.
- **What did not run there:** the checks after the bar check, which are the store disk and `efivar`.
- **The last whole run of the x86 boot suite** was on 3bed183b (351-KRN-0167).

The gate runs the whole suite on the candidate.
