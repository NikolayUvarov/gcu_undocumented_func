# Boot images: signed manifests, verification, provenance

**Version:** 0.1 (2026-10-08) · **Track:** `UPD` ([TRACKS.md](../../TRACKS.md)), main task [350](../../issues/350-signed-boot-images.md) · **Roadmap:** track C · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) Article 9, MC-3.11

This document describes how a boot volume is signed and checked, as built. Slots A and B, the boot records and the trial are in [slots.md](slots.md) (351-UPD-0006). What the platform guarantees is stated in the profile ([docs/profile](../profile/README.md), row "Article 9"). Anything not marked implemented here is plan (MC-12.3).

## The manifest — implemented (350-UPD-0002)

The build signs each boot volume it stages. `02_build.sh` signs `usb_root/` and `scripts/build_aarch64.sh` signs `aarch64_root/`, both with `scripts/sign_manifest.py`. Signing writes two files:

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

`scripts/sign_manifest.py` carries its own Ed25519, checked against RFC 8032's test vectors and, where it is installed, against the `cryptography` library (`tests/manifest_test.py`). With `--verify VOLUME PUBLIC` it checks a volume as the bootloader does.

## Verification at boot — implemented (350-UPD-0003)

The UEFI bootloader (`bootloader/src/verify.rs`) checks the volume before it loads anything:
1. It reads `MANIFEST.SIG` and `MANIFEST`, and checks the signature against the public key built into it (`verify_strict` of `ed25519-dalek`). A missing file or a bad signature stops the boot.
2. It reads each image it will use (the kernel and every boot service on the volume: 30 images on x86, 27 on aarch64), and checks its size and SHA-256 against the manifest's line before it parses or keeps it. An image the manifest does not list, or lists otherwise, stops the boot.

A stop prints `BOOT ERROR: <file>: <reason>` on the serial line and the UEFI console, and the machine halts. The reasons are `file not found`, `bad signature`, `another manifest format`, `not in the manifest` and `not as the manifest says`.

On success it prints the **launch record** on the serial line: `BOOT: MANIFEST <first 16 hex digits of the manifest's SHA-256> KEY <hex16> [(THE TEST KEY)] VERIFIED, <n> IMAGES CHECKED`. This is evidence of what was checked, not an authority (350-UPD-0004). The running system cannot read it yet: that needs a field in `BootInfo` and a `STAT` class, which is kernel work ([requests-KRN.md](../../issues/requests-KRN.md)).

Applications are listed in the manifest, but the loader reads them from the boot volume later and does not check them yet. They are covered by the signature only as far as an offline check (`--verify`) goes.

## Trust model

- **The anchor is the bootloader's built-in public key.** `bootloader/build.rs` embeds the key file named by `$MIND_BOOT_PUBLIC_KEY`, or the test key `bootloader/keys/test.pub`.
- **Without Secure Boot, nothing checks the bootloader itself.** The firmware runs whatever `BOOTX64.EFI` or `BOOTAA64.EFI` is on the volume. Whoever can write the boot volume can replace the bootloader, and with it the key. Verification alone therefore detects accidental change and changes to images or the manifest, not an attacker with write access to the whole volume. With Secure Boot and the project's own keys, the firmware runs only our signed bootloader: see [secure-boot.md](secure-boot.md) (351-UPD-0012, tested in QEMU, not yet on hardware).
- **The test key is public.** Its seed is the SHA-256 of a fixed text in `scripts/sign_manifest.py`, so anyone can sign with it. Builds made without a release key carry it, and the launch record says `(THE TEST KEY)`. A release is signed with a private seed kept outside the repository (`$MIND_BOOT_SIGNING_KEY`), and its bootloader is built with the matching public key. Key roles, rotation and revocation are [351-UPD-0009](../../issues/351-UPD-0009-rollback-policy-and-key-roles.md).
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
