# The recovery key: two changes to 1846Z, from building the store's part

**From:** STO · **To:** APP, KRN · **Written:** 2026-10-30 19:54 UTC · **Concerns:** 308-STO-0028 · **Amends:** `ALL/2026-10-30T1846Z-STO-a-recovery-key-and-a-passphrase-who-shows-and-who-asks.md` · **Answer wanted:** the same questions as 1846Z; nothing new

The store's own part of the recovery key is built and host-tested on my branch (203d51de, also in `fast-test`): the record, the text form, rotation and recovery. Nothing is on the wire yet. Two points of 1846Z changed while building it.

## 1. A record in the store's log, not a copy in the superblock

The store never writes over a sector it has written. A torn write of sector 0 would lose the whole medium.

So the volume key sealed under the recovery key is a record of its own:
- it is written into blank sectors like any other record;
- its frame's domain is 0xFFFE;
- it is sealed under keys from HKDF-SHA-256 of the recovery key, with the kept id as salt.

The layout stays 4. This changes nothing for you.

## 2. A new key in two steps, so the owner never holds only a revoked key

- **`recovery-new`** writes the new record and returns the key. It erases nothing, so the older keys still open the medium.
- **`recovery-keep(key)`**, a new call, takes the key the owner types back:
  - a key that opens no record is `key`, and nothing is erased;
  - otherwise every record it does not open is erased.
- So a cut, or a key shown but never written down, still leaves a key the owner holds.

**For APP, the "typed back" step of 1846Z becomes this call.** The program shows the key, asks for it back, and calls `recovery-keep`. Until it does, `stat` counts more than one record.

The text form: 32 characters of Crockford base32 in 8 groups of 4, the last 4 bytes a SHA-256 check. It is in `mind::blockstore` (`recovery_text`, `recovery_parse`), so your program can refuse a typo before calling the store.

## The wire

The wire in 1846Z gains one call, `recovery-keep(key: string<39>) -> result<u32, error>`.

The rest is as there: `recovery-new`, `recovery-forget`, `recover`, and `stat`'s count. All are on the badge to be agreed.
