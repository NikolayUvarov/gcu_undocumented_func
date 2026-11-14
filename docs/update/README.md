# Boot images: signed manifests, verification, provenance

**Version:** 0.2 (2026-11-13): the boot keys are the key file's, checked under the root keys built in (351-UPD-0029); 0.1 (2026-10-08) · **Track:** `UPD` ([TRACKS.md](../../TRACKS.md)), main task [350](../../issues/350-signed-boot-images.md) · **Roadmap:** track C · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) Article 9, MC-3.11

This document describes how a boot volume is signed and checked, as built. Slots A and B, the boot records and the trial are in [slots.md](slots.md) (351-UPD-0006). What the platform guarantees is stated in the profile ([docs/profile](../profile/README.md), row "Article 9"). Anything not marked implemented here is plan (MC-12.3).

## The manifest — implemented (350-UPD-0002)

The build signs each boot volume it stages. `02_build.sh` signs `usb_root/` and `scripts/build_aarch64.sh` signs `aarch64_root/`, both with `scripts/sign_manifest.py`. Signing writes the manifest, its signature and the key file:

- **`MANIFEST`**, text in UTF-8 with lines ending in LF and single spaces between fields:

  | Line | Meaning |
  |---|---|
  | `MIND-MANIFEST 1` | the format and its version; another version is refused, never read as this one |
  | `key <hex16>` | the first 16 hex digits of the SHA-256 of the public key that signs it |
  | `commit <hex>` | the git commit the build was made from (`unknown` outside a checkout) |
  | `toolchain <channel>` | the Rust toolchain of `rust-toolchain.toml` |
  | `inputs <sha256>` | one SHA-256 over the path and SHA-256 of every crate's `Cargo.lock`: the locked dependencies |
  | `file <path> <size> <sha256> <flags> <MiB>` | one per shipped file, sorted by path: the bootloader, the kernel, every service and program, the licences, the voice model. `flags` and `MiB` are what a program's `.mind_request` section asks for, 0 for the rest |

  A manifest only describes. The requests it lists are the ones a program asks its launcher for, and they grant nothing (MC-3.11): the shell and the loader decide what to lend, as before.
- **`MANIFEST.SIG`**: an Ed25519 signature (RFC 8032), 64 bytes, over the manifest's exact bytes.
- **`KEYS`**, the key file whose `boot` role names the key that signs the manifest ([publishing.md](publishing.md), 351-UPD-0026), and **`KEYS.1`, `KEYS.2`, …**, the key files before it at which the root changed (351-UPD-0029). They are the last of the key history in the directory `$MIND_KEYS` (`1`, `2`, … as a server's `keys/` holds them), or the test key file. A build signed with a boot key other than the test one and no `$MIND_KEYS` is refused: its key file would not name the key.

`scripts/sign_manifest.py` carries its own Ed25519, checked against RFC 8032's test vectors and, where it is installed, against the `cryptography` library (`tests/manifest_test.py`). With `--verify VOLUME PUBLIC` it checks a volume as the bootloader does.

## Verification at boot — implemented (350-UPD-0003)

The UEFI bootloader (`bootloader/src/verify.rs`) checks the volume before it loads anything:
1. It reads `KEYS` and checks it under the root keys built into it (351-UPD-0029, `bootloader/src/keychain.rs`): each of `KEYS.1`, `KEYS.2`, … in turn, then `KEYS`, signed by the threshold of the root trusted before it (the built-in set first) and by the threshold of its own `root` role, their versions rising. In a slot, the version of `KEYS` must not be below the one the boot records name ([slots.md](slots.md)). A missing file, a file short of either threshold, or a version too low stops the boot with `BOOT ERROR: KEYS: <reason>`; the serial line says which file and which threshold. The bootloader has no clock it trusts, and does not read the key file's expiry: the updater does.
2. It reads `MANIFEST.SIG` and `MANIFEST`, and checks the signature against the keys of the key file's `boot` role (`verify_strict` of `ed25519-dalek`). A manifest carries one signature, so a `boot` role whose threshold is above 1 is refused. A missing file or a bad signature stops the boot.
3. It reads each image it will use (the kernel and every boot service on the volume: 34 images on x86 and 30 on aarch64, the `updater` among them), and checks its size and SHA-256 against the manifest's line before it parses or keeps it. An image the manifest does not list, or lists otherwise, stops the boot. A boot service missing from the volume stops it too if the manifest lists it; one the manifest does not list (a service of the other architecture) is left out, and `init` does not start it (351-KRN-0022).

A stop prints `BOOT ERROR: <file>: <reason>` on the serial line and the UEFI console, and the machine halts. The reasons are `file not found`, `bad signature`, `another manifest format`, `not in the manifest` and `not as the manifest says`, and for `KEYS`: `not a key file`, `fewer than the threshold of the root keys signed it`, `fewer than the threshold of its own root keys signed it`, `the versions of its key files do not rise`, and `below the key file version the boot records name`.

On success it prints the key file it took, `BOOT: <dir>KEYS: VERSION <n>, CHECKED UNDER THE ROOT KEYS BUILT IN [(THE TEST ROOT KEYS)]`, and the **launch record** on the serial line: `BOOT: MANIFEST <first 16 hex digits of the manifest's SHA-256> KEY <hex16> [(THE TEST KEY)] VERIFIED, <n> IMAGES CHECKED`; `(THE TEST KEY)` when the root set built in is the test one or the test boot key signed it. This is evidence of what was checked, not an authority (350-UPD-0004). The running system cannot read it yet: that needs a field in `BootInfo` and a `STAT` class, which is kernel work ([requests-KRN.md](../../issues/requests-KRN.md)).

Applications are listed in the manifest, but the loader reads them from the boot volume later and does not check them yet. They are covered by the signature only as far as an offline check (`--verify`) goes.

## Trust model

- **The anchor is the root set built into the bootloader** (351-UPD-0029). `bootloader/build.rs` embeds the root set's public keys and threshold from the file `$MIND_ROOT_KEYS` names (`threshold T`, then a key a line, as `release.py keys root` prints it), or the test set `updater/keys/root-test.pub`; the updater is built with the same set. The boot keys are the key file's, so a boot key is changed or revoked with a new key file, not a new bootloader. The root set built in is changed only with a new bootloader at `EFI/BOOT/` (and the updater); a root rotated through the key files is followed through the files of its changes.
- **What the bootloader holds is a version, not a history.** A slot's key file is refused below the version the boot records name, which the updater raises when it confirms a slot under a newer key file ([slots.md](slots.md)). Whoever holds the root keys of a version, or the set built in, can make a chain of key files from it; the bootloader cannot tell that chain from the real one above the version the records name. The updater, which keeps the key file it trusts, takes only the next version signed by that one's root (351-UPD-0027).
- **Without Secure Boot, nothing checks the bootloader itself.** The firmware runs whatever `BOOTX64.EFI` or `BOOTAA64.EFI` is on the volume. Whoever can write the boot volume can replace the bootloader, and with it the key. Verification alone therefore detects accidental change and changes to images or the manifest, not an attacker with write access to the whole volume. With Secure Boot and the project's own keys, the firmware runs only our signed bootloader: see [secure-boot.md](secure-boot.md) (351-UPD-0012, tested in QEMU, not yet on hardware). A slot's own loader (351-UPD-0010) is checked like a slot's images: the loader at `EFI/BOOT/` starts it only as the slot's signed manifest lists it ([slots.md](slots.md)).
- **The test key is public.** Its seed is the SHA-256 of a fixed text in `scripts/sign_manifest.py`, so anyone can sign with it. Builds made without a release key carry it, and the launch record says `(THE TEST KEY)`. A release is signed with a private seed kept outside the repository (`$MIND_BOOT_SIGNING_KEY`), and the key file it carries (`$MIND_KEYS`) names that seed's public half in its `boot` role. Key roles, rotation and revocation are [publishing.md](publishing.md).
- **What a signature says** (MC-9.2): that the key's holder authorized these exact bytes. It does not say they are free of errors.
- **The test harness** signs every volume it builds with the build's key, the test key in CI. Its refusal cases change a file without signing again.

## Reproducible builds — implemented as a check (350-UPD-0001)

`scripts/reproducible.sh` builds one commit twice, each time from a fresh checkout at the same path with the pinned toolchain and the locked dependencies, and compares every staged file byte for byte, `MANIFEST` and `MANIFEST.SIG` included. Any difference fails the check. `scripts/ci_local.sh` runs it for x86 as the group "x86: reproducible build".

The conditions that affect the result (MC-9.7):
- the commit;
- the toolchain of `rust-toolchain.toml`;
- every `Cargo.lock`;
- the architecture;
- the signing key;
- **the checkout path**.

Every program depends on `libmind` by a relative path that leaves the program's own Cargo workspace. Cargo hashes such a dependency's absolute path into each crate's metadata, and that metadata names symbols and steers code layout. So two builds of one commit at different paths differ in most programs, while two builds at the same path are identical. `--other-path` builds once more at another path and lists what differs. The absolute paths of the build machine also appear in panic messages of `libmind` code and in Cargo's registry paths.

Not provided:
- builds that are identical across checkout paths (one Cargo workspace, or path remapping and stable crate metadata);
- a record of the host's own tools beyond the Rust toolchain (`python3`, `mtools`);
- a check on GitHub CI (the local gate runs it).
