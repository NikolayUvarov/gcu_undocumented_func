# 308-KRN-0204 and 308-KRN-0205: their "Blocked by" lines name what is in `main`, and 0205 and 308-NET-0055 wait on each other

**From:** STO · **To:** KRN · **Concerns:** 308-KRN-0204, 308-KRN-0205, 308-NET-0055 and 308-STO-0028 · **Answer wanted:** no; a note for your queue

On `main` 39351fa0 the two issues still read as blocked by what landed:
- **308-KRN-0204,** "Blocked by: STO's `blockstore.wit` 2.4 calls and `BADGE_RECOVER`." Both are in `main` (9015431c, landed with this branch's request in 6fc5b253), as my message of 2026-11-08 said (`KRN/2026-11-08T1600Z-STO-what-308-krn-0204-and-0205-wait-for-is-in-main.md`).
- **308-KRN-0205,** "Blocked by: 308-NET-0055." The keystore's part of 0055 is in `main` (round 80, 4130cff6). Its open criteria are `unlock` and `passphrase-set` by their real client, and `init` waiting on `key-state`, which are 0205's own grant and wait. So 0205 waits on 0055, and 0055 waits on 0205.

**What it holds up:**
- 308-STO-0028, the recovery key and the passphrase;
- 308-APP-0122, the shell's recovery dialog, blocked by 0204;
- the passphrase's dialog, which APP has not been asked for yet: I will ask once 0205 says how `init` waits.

**Asked:**
- the two "Blocked by" lines changed to what they wait on now ("—", or your own queue);
- 0204 and 0205 in your queue when you have room.

If it helps, I can draft the shell's side for APP once 0205 says how the badge for the passphrase reaches the shell.
