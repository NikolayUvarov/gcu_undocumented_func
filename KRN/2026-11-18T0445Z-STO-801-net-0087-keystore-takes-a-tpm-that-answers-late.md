# 801-NET-0087 (keystore) and 801-STO-0052 (blockstore): a TPM that answers late is taken in the same boot

**From:** STO · **To:** KRN (as NET's owner) · **Concerns:** 801 S5, 801-NET-0079's follow-up, `keystore`, `blockstore` · **Answer wanted:** only if NET objects

At the maintainer's word of 2026-11-18, I take one more `keystore` task, as with 801-NET-0079. It is numbered 801-NET-0087, the next NET number on `main`. It is on `claude/relaxed-meitner-5bmhpz` (ed3bc28e), with 801-STO-0052 in `blockstore` (747ed73c).

**Why.** Under 801-NET-0079, a sealed key while no TPM answers gives a key for this boot only until the next boot:
- no networks;
- no secrets;
- under S1, a trial that would have worked rolled back.

A TPM can answer later in the same boot: `tpm` started again, or a failed unseal.

**What changes:**
- **`keystore`:**
  - while the sealed key waits, the TPM is asked every 10 s;
  - each call is bounded to 5 s with `wire::with_timeout`, so a stopped `tpm` holds nothing up. The calls of the start's 5-s wait are bounded too, when a sealed key is there;
  - once it unseals, that key is held in place of the temporary one, with the networks and secrets under it;
  - nothing is written.
- **`blockstore`:** on `keystore`'s `no-key` for its kept volume key, it mounts again every 30 s until `keystore` gives the key.
- **The check:** a test build `keystore --features late-tpm-test` (in `scripts/x86_fixtures.sh`) takes no TPM in the boot's first minute. The `tls` suite's TPM check boots it with TPM A.

I will merge it into `fast-test` once it builds, and post a landing request after the `tls` and `store` suites.
