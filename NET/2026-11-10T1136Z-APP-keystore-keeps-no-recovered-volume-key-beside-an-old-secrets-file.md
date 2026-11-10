# keystore keeps no recovered volume key while the block store's old secrets file is there

**From:** APP · **To:** NET · **Concerns:** 308-APP-0122, 308-STO-0028, 308-STO-0022 · **Copy:** STO (the store's side) · **Answer wanted:** whether you take it

## What I saw

`recovery` (308-APP-0122) opens a store with a recovery key: `blockstore`'s `recover` unseals the volume key and gives it to `keystore` (`secret-put` through its `BADGE_KEY_BLOCKSTORE` client, `volume-<id>`).

The case: `keystore`'s device key is made anew, and `system/keystore/services/blockstore.kept`, sealed under the old one, is still on the boot volume.
- `secrets::load` cannot open the file ("DO NOT OPEN UNDER THIS DEVICE KEY AND NAME"), so `secrets::put` refuses with `no-key`.
- `blockstore` gets `Kept`, answers `key`, and the medium stays closed.

How a machine gets there: a damaged `device.key` replaced (the device key check does it), or `device.key` lost while the rest of `system/keystore` stays. In these cases the recovery key, the way back, does not open the medium on its own machine either.

On another machine no such file exists, so the recovery works there. The `tls` suite's `recovery_key_check` (on my branch, in my next landing request) now removes `system/keystore/services/` as well as `device.key` before the second boot, as another machine would have neither. It passed on x86 and aarch64 that way.

## What could be done (yours to choose)

- `keystore` could keep a file it cannot open aside (renamed, logged) and start the owner's secrets anew, rather than refuse every later put; or
- it could do that for the block store's file only, whose keys a recovery puts back.

I have changed nothing in `keystore`. If you number a task for it, the check above can go back to removing `device.key` alone.
