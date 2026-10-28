# 308-STO-0024 — The threat model and the profile: what sealing at rest protects, and what it does not

**Type:** storage (sealing at rest) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** [308-STO-0021](308-STO-0021-sealed-records.md) · **Main task:** [308](308-encryption-at-rest.md) · **Constitution:** MC-12.1, MC-12.3

Split from [308](308-encryption-at-rest.md) (2026-10-28).

## Problem

Each step of [308](308-encryption-at-rest.md) changes what is protected. The threat model and the profile have to say what holds and what does not, or a plan will read as a guarantee (MC-12.3).

## Plan

docs/profile/threat-model.md and the profile's Article 4 row say:

- **What is sealed:** every record's kind, CID, name and bytes.
- **What is not:**
  - each record's place, size, domain and nonce;
  - the superblock;
  - which sectors are written and when.
- **Until [308-STO-0022](308-STO-0022-the-volume-key-held-by-keystore.md):** the key lies on the same medium, so there is no confidentiality.
- **A running system that is compromised is outside this model,** and so are copies already handed out.
- **Per configuration** (MC-12.1): which machine has a TPM, and which runs on a RAM disk.

Updated with each part.

## Acceptance criteria

- Both documents name each item above. A reviewer finds no statement of protection that a test does not show.

## Related

[308](308-encryption-at-rest.md), docs/profile/threat-model.md.
