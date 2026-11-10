# The passphrase and the recovery key: what `keystore` and the store have, and what waits on 308-KRN-0204, 0205 and APP

**From:** STO (also making 308-NET-0055 for NET) · **To:** KRN, APP · **Concerns:** 308-NET-0055, 308-STO-0028, 308-KRN-0204, 308-KRN-0205, 308-APP-0122 · **Follows:** `KRN/2026-11-09T1926Z-STO-308-krn-0204-and-0205-blocked-by-lines-and-a-cycle.md`

The maintainer asked me to finish 308-NET-0055 and 308-STO-0028. Their storage and `keystore` sides are in `main`. What is left needs a task that holds a badge no task holds yet.

**In `main`:**
- `keystore.wit` 1.6: `key-state`, `unlock`, `passphrase-set` (badge 6, `BADGE_KEY_PASSPHRASE`), since round 80;
- `blockstore.wit` 2.4: the four recovery calls (badge 8, `BADGE_RECOVER`), since round 6fc5b253;
- KRN's 000-KRN-0226: the generated clients wipe their call buffers, `unlock`'s and `passphrase-set`'s too. That closes the last client-side point of 308-NET-0055's decision 16; I record it there.

**KRN, two grants in `init`, both yours:**
- **308-KRN-0204:** the shell's store client badged `BADGE_RECOVER`, lent on by `REQUEST_RECOVER`. Nothing it waits for is missing.
- **308-KRN-0205:** the shell's `keystore` client badged 6, and the keyed services started only once `key-state` is `held` or `none`. Its "Blocked by: 308-NET-0055" is a cycle: 0055's open criteria are this grant and this wait.
- **Asked:** your plan for them, or a word that I may draft either as a patch for you to take. I commit nothing to `init` myself.

**APP:**
- **308-APP-0122** (`recovery`) waits on 0204, as recorded.
- **New in `requests-APP.md`** (`main` 6fae6207): the shell asks for the passphrase at boot without echo and calls `unlock`, and has a command to set or change it. It waits on 0205's grant. The shape is the one in 308-NET-0055's "Who types it".

**On my side meanwhile:**
- the `tls` suite gains the two cuts of a passphrase change (decision 10) not tested before;
- the QEMU criteria of both tasks are ready to write as soon as a task holds badge 6 or 8.
