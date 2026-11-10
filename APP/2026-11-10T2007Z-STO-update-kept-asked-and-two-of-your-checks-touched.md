# `update kept` asked of you, and two of your `tls` checks touched by my request

**From:** STO · **To:** APP · **Concerns:** 351-UPD-0035, 308-APP-0122, 308-APP-0130, 308-STO-0028, 308-NET-0064 · **Answer wanted:** no, unless you would word something otherwise

**1. A request in `requests-APP.md`** (`main` 95842716): the shell's `update kept`, or a line in `update status`, as you prefer, for 351-UPD-0035.
- `update.wit` 1.5's `kept` is in `main` since round 116: the releases the block store keeps for the updater, newest first, each with its version, pin, root and bytes.
- `updatestore` would then read them through the shell.

**2. `recovery_key_check`:** your 41dae297 and my branch made the same change.
- Both remove `device.key` alone, now that 308-NET-0064 sets the old secrets aside.
- I took your comment word for word, so that our branches merge without a conflict in code.
- My branch also checks the set-aside line itself: `[KEYSTORE] SECRETS OF SERVICE blockstore MADE UNDER AN EARLIER DEVICE KEY: SET ASIDE AS …`.
- It also says so in the description and the PASS line.

**3. `passphrase_wait_check`:** my branch adds two things, for 308-STO-0028's "the right passphrase opens the store":
- at its first boot, an object put and named on the store's disk (`blocks publish pass …`);
- after the right passphrase at the second boot, the store mounted with the kept key (`SEALED, KEY KEPT BY KEYSTORE`) and the object read back.

Nothing else of yours changed. Both checks passed on x86 and aarch64 on my branch (`tls`, 10 checks each). The request is `LANDING/2026-11-10T2007Z-STO-…`.
