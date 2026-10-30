# 308 — Encryption at rest, and the key's lifecycle, before any personal data

**Type:** main task · **Owner:** `STO` track · **Priority:** P1 · **Status:** open · **Blocked by:** [307](307-the-stores-own-partition.md) (a medium worth sealing) · **Roadmap:** track B "encryption and key management before confidential data" · **Constitution:** MC-4.8, MC-6.8, MC-9.6, MC-11.9, Appendix B.3

From the kernel session's draft (`STO/2026-10-25T1557Z-KRN-draft-plan-the-store-as-a-minds-memory.md` on `internal-dev-messages`), taken by the storage session at the maintainer's word (2026-10-27). Everything here is plan, not a guarantee (MC-12.3).

## Problem

Appendix B.3: "encryption and key management are designed before confidential data is placed, not added after it has been disclosed." A stick or a disk read in another machine gives away everything the store holds today. The device key itself is stored unencrypted (docs/profile/threat-model.md). The threat here is the medium read elsewhere; a running system that is compromised is outside this task, and the threat model says so.

## Plan

- **Sealed records, with a key per memory domain** (the storage session's choice, 2026-10-27, over a sealed medium below the store): only per-domain keys let a domain be forgotten for sure by destroying its key (MC-4.8), which a sealed medium cannot do for one domain.
  - Each record sealed with an AEAD (ChaCha20-Poly1305) and a nonce of its own, bound to its place so a moved record is refused.
  - Names, pins and the index sealed too: a CID of predictable content tells what the content is (RFC 6920, Appendix G).
  - The CID stays the digest of the plain bytes, so identities do not change with keys.
- **Forgetting stays three operations** (MC-4.8): deleting data, deleting references and destroying keys. A domain whose key is destroyed is unreadable, then collected.
- **The volume key** wraps the domain keys:
  - sealed by the TPM where there is one (351-NET-0006, waiting for the TPM's registers from `KRN`), else unlocked by a passphrase at boot;
  - a recovery key the owner keeps;
  - rotation and destruction written down (MC-9.6);
  - keys held by `keystore` (`NET`, 000-NET-0045); the store asks it to wrap and unwrap, and never holds the volume key's material longer than a mount (MC-11.9).
- **The recovery set of [305](../issues-done/305-recovery-without-the-store.done)** depends on none of it (MC-6.8).

## Tasks

- [308-STO-0021](../issues-done/308-STO-0021-sealed-records.done): sealed records, layout 3, ChaCha20-Poly1305; the key in the clear on the medium for now (no confidentiality yet). Done 2026-10-29.
- [308-STO-0022](../issues-done/308-STO-0022-the-volume-key-held-by-keystore.done): the volume key held by `keystore`, under the device key the TPM seals. Done 2026-10-30.
- [308-STO-0023](../issues-done/308-STO-0023-memory-domains.done): memory domains, a key per domain, forgetting by destroying it. Done 2026-10-29.
- [308-STO-0024](308-STO-0024-what-sealing-protects.md): the threat model and the profile.
- [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md): a recovery key and a passphrase for the volume key, with `APP` and `KRN`.

## Acceptance criteria

- Host: a forged or moved record refused; the wrong key refused; a domain whose key is destroyed is unreadable, then collected.
- QEMU, x86 and aarch64: the store on a sealed medium across a reboot; a stick read without the key shows no content, names or CIDs.
- docs/profile/threat-model.md and docs/storage updated.

## Related

[307](307-the-stores-own-partition.md), [312](312-the-minds-memory.md), 351-NET-0006, 351-DRV-0015, 000-NET-0045.
