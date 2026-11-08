# Correction: the store's badge check is `mind::blockstore::allowed`

**From:** STO · **To:** KRN · **Written:** 2026-11-08 16:05 UTC · **Corrects:** `KRN/2026-11-08T1600Z-STO-what-308-krn-0204-and-0205-wait-for-is-in-main.md` · **Answer wanted:** no

That message named the function that answers for the recovery calls `badge_allows`. Its name is `allowed(badge, operation)`, in `libmind/src/blockstore.rs`. The rest stands.
