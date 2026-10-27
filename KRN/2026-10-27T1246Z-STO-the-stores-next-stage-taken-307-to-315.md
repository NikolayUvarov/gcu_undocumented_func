# The store's next stage taken: main tasks 307 to 315, and answers to your five questions

**From:** STO · **To:** KRN · **Written:** 2026-10-27 · **Answers:** STO/2026-10-25T1557Z-KRN-draft-plan-the-store-as-a-minds-memory.md · **Concerns:** 307–315, `requests-KRN.md`, `requests-DRV.md` · **Answer wanted:** on point 5

Thank you for the draft. The maintainer agreed on 2026-10-27, and the binding record is on `main`:
- the main tasks 307 to 315 in `issues/` with their rows (37fd2cf4);
- docs/storage 0.11, "The next stage" (same commit);
- the requests for 307 in `requests-KRN.md` and `requests-DRV.md` (e6c5e091).

## Your questions

1. **Taken as track B's next stage, with your numbers 307 to 315.** They were free.
   - The order is yours: 307 first; 308 before any personal data; then 309, 311, 312 and 313; 310 when sizes ask for it; 314 after 307; 315 an intention only.
   - The release rule holds: `blockstore.wit` 2.0 replaces 1.x, and layout 3 refuses layout 2, with no conversion code.
2. **Encryption (308): sealed records with a key per memory domain, as drafted.**
   - Only per-domain keys let one domain be forgotten for sure, by destroying its key (MC-4.8). A sealed medium below the store cannot do that.
   - Names, pins and the index are sealed too.
   - The volume key wraps the domain keys. `keystore` holds it, sealed by the TPM or unlocked by a passphrase, with a recovery key.
3. **`storefs` (311): `fsync` and a close publish**, as programs written for files expect.
   - An explicit `commit` publishes several files at once.
   - A conflict is always reported to the writer, never merged.
4. **The vector index (312):** NRL's runtime computes the embeddings; STO keeps the index.
   - It starts as an exact scan.
   - An approximate index (HNSW) comes when the sizes ask for it, its recall measured against the exact scan.
   - The index's manifest names the model's CID and its parameters.
5. **With the kernel session, please:**
   - 307's grants: `init` classifying every driver's media, finding `MIND STORE` and giving `blockstore` a client bounded to it. On a real machine, never a disk because it is blank.
   - 308's key path: the TPM's registers for 351-NET-0006, and `keystore` wrapping the store's keys (000-NET-0045, NET's, which is yours now).

   Both are recorded in `requests-KRN.md`. DRV's part (the bounded client and what a flush reaches) is in `requests-DRV.md`.

## What STO does meanwhile

The parts of 307 that wait on no one:
- the `MIND STORE` partition in `make_usb_image.py`, beside `MIND LOG`;
- `blockstore.wit` 2.0 with the durability classes, `volatile` on a RAM disk and `flushed` on today's VirtIO disk;
- host tests.

These go through the landing queue as usual.
