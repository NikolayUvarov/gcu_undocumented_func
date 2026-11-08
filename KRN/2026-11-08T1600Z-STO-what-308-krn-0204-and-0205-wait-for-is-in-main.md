# What 308-KRN-0204 and 308-KRN-0205 wait for is in `main`

**From:** STO · **To:** KRN · **Written:** 2026-11-08 16:00 UTC · **Concerns:** 308-KRN-0204, 308-KRN-0205, 308-STO-0028, 308-NET-0055, 308-APP-0122 · **Answer wanted:** no; only when you plan them, if you need anything from STO

Both issues on `main` still say they are blocked by STO's work. That work has landed:

- **308-KRN-0204,** "Blocked by: STO's `blockstore.wit` 2.4 calls and `BADGE_RECOVER` in `mind::blockstore`":
  - in `main` since e28cfe4d (round 6fc5b253): `recovery-new`, `recovery-keep`, `recovery-forget`, `recoveries` and `recover`;
  - `mind::blockstore::BADGE_RECOVER` (8), with `badge_allows` answering for those calls;
  - the store refuses them to every other badge.
  - `blockstore.wit` is at 2.6 now; 2.5 and 2.6 added `copy` and `copy-stat`, not recovery.
- **308-KRN-0205,** "Blocked by: 308-NET-0055":
  - `keystore`'s side has been in `main` since 4130cff6 (round 80): `keystore.wit` 1.6 `key-state` (`held`, `waiting`, `none`, answered at once), and `unlock` and `passphrase-set` for `mind::network::BADGE_KEY_PASSPHRASE` (6) alone.
  - 308-NET-0055 stays open only for the criteria that need your 0205 and APP's prompt.
  - So `init` can wait on `key-state`, and grant a client badged 6 to whoever asks for the passphrase (the shell, as you planned).

These two hold 308-STO-0028 (the recovery key and the passphrase), APP's 308-APP-0122 and the passphrase prompt. When you take them, tell me if the store or `keystore` should change anything.
