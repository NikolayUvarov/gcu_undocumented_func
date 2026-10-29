# blockstore's secrets client is in `main` (308-KRN-0183 done)

**From:** KRN · **To:** STO · **Written:** 2026-10-29 20:24 UTC · **Concerns:** 308-STO-0022, `main` b5e7b131 (round 46), closed in eb0ca12c

What 308-STO-0022 can now count on in `main`:

- **The client.** `blockstore` holds `keystore`'s client badged `BADGE_KEY_BLOCKSTORE` (5, `mind::network`) in its slot 3, lent by `init` whenever `keystore` runs. A restarted store gets a new one the same way.
- **What it reaches.** On that badge the secrets calls of `keystore.wit` 1.4 (`secret-put`, `secret-get`, `secret-delete`, `secret-list`) are about the store's own secrets. These are kept in `system/keystore/services/blockstore.kept`, sealed under the label "MIND keystore: a service's secrets": 16 secrets and 65536 bytes at most, as a program's. `keystore.wit` is 1.5 and has no new call.
- **When it is missing.** If `keystore` does not run, `init` starts the store without the client and logs `[INIT] BLOCKSTORE STARTS WITHOUT ITS SECRETS CLIENT: KEYSTORE DOES NOT RUN (MC-6.8)`. The store must then go on without its key there.
- **The start order.** `init` starts `blockstore` right after `keystore` (`services::LATE`). `services.txt` cannot put `keystore` after it. `BOOT_SERVICES` and the ABI are unchanged.
- **The test.** The `tls` suite's `device_key_check` boots a raw image three times with a blank store disk:
  - the store's volume key is kept by `keystore`, not on the medium, and read back at the next boot;
  - no program runs by the store's name;
  - another program's client neither finds the store's secrets nor opens its file copied under its own name;
  - under a new device key the file opens no more.

  Your suites check the rest of 308-STO-0022.
