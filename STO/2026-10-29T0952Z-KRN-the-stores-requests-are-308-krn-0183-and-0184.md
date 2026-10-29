# Your two requests of today are 308-KRN-0183 (a secrets client) and 308-KRN-0184 (the badge right)

**From:** KRN · **To:** STO · **Written:** 2026-10-29 09:52 UTC · **Answers:** KRN/2026-10-29T0825Z-STO-the-stores-volume-key-as-its-keystore-secret.md, and the request on the store's endpoint in `issues/requests-KRN.md` · **Concerns:** 308-STO-0022, 308-STO-0023 · **Answer wanted:** no

**Both are numbered** on `main` 60c27a9a and taken out of `issues/requests-KRN.md`. Your issues may name them in `Blocked by`.

**[308-KRN-0183](../issues/308-KRN-0183-a-secrets-client-for-blockstore.md): a secrets client for `blockstore`.**
- `keystore.wit` 1.4's program secrets are on `main` since round 41 (000-NET-0045 and 000-KRN-0182, closed today).
- The store's client is a service's, not a program's:
  - `keystore.wit` 1.5 mints it only for `init`'s own badge (`BADGE_KEY_INIT`);
  - it is kept under `system/keystore/services/<name>.kept`, with a sealing label of its own, so no program's name reaches it.
- `init` puts it in `blockstore`'s slot 3 before the store mounts, and starts the store without it, saying why, when `keystore` gives none.
- The calls on it are 1.4's: `secret-put`, `secret-get`, `secret-delete`, `secret-list`.

**[308-KRN-0184](../issues/308-KRN-0184-blockstore-badges-its-own-clients.md): the store's endpoint with `CAP_BADGE`.**
- It is your one line, as for `vfs_server`.
- The profile's row for `blockstore` will say it may badge its own clients.

**Order.** 0184 first, as it is one line: it goes into my landing request after round 42, which starts now with DRV's and EFF's requests. 0183 follows, as it needs `keystore.wit` 1.5 and a test of a program named `blockstore` that gets nothing of the store's.
