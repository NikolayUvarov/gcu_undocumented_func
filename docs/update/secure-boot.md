# Secure Boot with our own keys

**Version:** 0.1 (2026-10-08) · **Track:** `UPD`, task [351-UPD-0012](../../issues/351-UPD-0012-secure-boot-with-our-own-keys.md) · **Constitution:** MC-9.1, 9.4, 9.6 · Russian: [secure-boot_RU.md](secure-boot_RU.md)

The bootloader checks the kernel and the services against a signed manifest ([README.md](README.md)), but nothing checks the bootloader. With Secure Boot on and the project's own keys enrolled, the firmware runs only a bootloader signed with our db key, and not one whose hash is in dbx. That is what makes the bootloader's checks count against someone who can write the disk. Tested in QEMU; not yet on a real machine.

## The keys

UEFI keeps three kinds of key:
- **PK**, the platform key: it authorizes changes to KEK;
- **KEK**: it authorizes changes to db and dbx;
- **db**: it signs bootloaders.

dbx lists what is revoked, here the Authenticode hashes of old bootloaders.

`scripts/secure_boot.py keys DIR` makes an RSA-2048 PK, KEK and db labelled TEST. Real keys are made the same way on a machine kept apart. They never enter the repository, and they are not the boot key or the release key (MC-9.6). The private PK and KEK are needed only to enrol or change keys, so keep them offline. The db key signs every release's bootloader.

## Signing the bootloader

With `MIND_SECURE_BOOT_KEYS=DIR` (a directory holding `db.key` and `db.crt`), `02_build.sh` and `scripts/build_aarch64.sh` sign the bootloader with `sbsign` before they sign the boot manifest. The manifest then lists the signed bootloader. By hand: `scripts/secure_boot.py sign BOOTX64.EFI DIR OUT`.

## Revoking a bootloader

Revocation is chosen to be **dbx**: the Authenticode SHA-256 of a revoked bootloader (`scripts/secure_boot.py hash OLD.EFI`) goes into dbx. On QEMU that means a variable store made with `secure_boot.py vars … --revoke OLD.EFI`. On a real machine it means an authenticated update of dbx, signed with our KEK, that the updater applies (351-UPD-0009, 0010). A generation number checked against a TPM counter, as shim's SBAT does, is not used: dbx needs no TPM and the firmware enforces it. The cost is that each revoked bootloader takes room in dbx.

## The trust model

- **The root of trust** is the firmware's Secure Boot with our keys.
- **What it does not cover:**
  - the firmware itself;
  - anyone who can change the firmware's settings: turn Secure Boot off, or enrol other keys. Set a firmware password;
  - anything signed with a key we enrol by choice. A machine with only our keys runs nothing Microsoft signs: no other operating system, and no option ROMs signed only by Microsoft, unless their keys or hashes are added deliberately.
- **The profile** (Article 9) states what has been tested: OVMF in QEMU.

## In QEMU

`tests/secure_boot_smoke.py` runs OVMF's Secure Boot build (`OVMF_CODE_4M.secboot.fd`) with a variable store made for the run, holding test keys, and checks that:
- the bootloader signed with our db key boots the system to the shell;
- the firmware refuses (`Access Denied`) the unsigned bootloader, one signed with another key, and the signed one once its hash is in dbx.

CI runs it in the "USB image" job, and `scripts/ci_local.sh` as "x86: Secure Boot with our keys".

## On a real PC (to be tried on the maintainer's machine)

1. **Make the keys** on a machine kept apart: `scripts/secure_boot.py keys /secure/mind-keys`. Copy `PK.crt`, `KEK.crt` and `db.crt` to a FAT USB stick; the firmware setup reads certificates from there (some read only `.cer`/DER: `openssl x509 -in PK.crt -outform der -out PK.cer`).
2. **Build and sign:** `MIND_SECURE_BOOT_KEYS=/secure/mind-keys ./02_build.sh`, then write the image as usual ([docs/write-disk.md](../write-disk.md)).
3. **In the firmware setup:** set a firmware (administrator) password. In the Secure Boot menu choose custom mode, delete all keys (this leaves setup mode), then enrol `db.crt` into db, `KEK.crt` into KEK and `PK.crt` into PK, **PK last**. Turn Secure Boot on.
4. **Check:** the system boots from the signed disk. A disk with an unsigned bootloader (a build without `MIND_SECURE_BOOT_KEYS`) is refused by the firmware.
5. **To boot another system again:** enrol its keys or hashes into db deliberately, or turn Secure Boot off.

The steps differ between firmware vendors. What the maintainer's PC does is recorded with its first run (211-PRT-0004 and [issues-human](../../issues-human/README.md)).

## Not provided yet

- a run on real hardware;
- authenticated dbx updates applied by the updater (351-UPD-0009, 0010);
- Secure Boot on aarch64. AAVMF's Secure Boot build is not tested: the bootloader is signed the same way, but no aarch64 test enrols keys;
- a firmware password or setting that MIND Core can check.
