# 750-UPD-0019 — Signed application packages: the contract, the key role and one check for the gateway and the loader

**Type:** format and library · **Owner:** `UPD` track · **Priority:** P1 · **Status:** open (contract draft 0.1 below, not agreed yet) · **Blocked by:** — for the contract, the routine and the tool; the loader's part waits for `KRN`, the install for 750-EFF-0008 · **Main task:** [750](750-effector-agent.md) · **Roadmap:** stage V · **Constitution:** MC-3.11, MC-9.2, MC-9.4, MC-9.8, MC-12.4, MC-12.7

From the request "Signed application packages: format, key role, verification (750)" in requests-UPD.md (recorded by the Effector agent track, 2026-10-09), and point 2 of [requests-EFF.md](requests-EFF.md) (the maintainer's handoff, 2026-10-15).

## Problem

- **What the server can do.** The Effector server installs software by sending a package (750-EFF-0008).
- **What MIND Core has for that.** MIND Core signs its boot image (`MANIFEST`, `MANIFEST.SIG`, the boot key: 350). It has:
  - no format for an application installed at run time;
  - no key that may sign one;
  - no check of an application when it is loaded (profile: "not met").
- **What must not happen.** A package must not become running code on the strength of TLS, or of a transfer hash, alone (MC-9.2, MC-9.8).
- **What Effector waits for.** It implements its server schema only after this contract is agreed (requests-EFF.md, point 2).

## Plan: the contract, draft 0.1 (a proposal to `EFF`, `KRN` and the Effector server project)

Nothing below exists yet as code. The contract becomes `docs/update/application-packages.md` 1.0, in English and Russian, when `EFF` (or the maintainer, while `EFF` has no owner), `KRN` and the Effector server agree on it.

**One package, one application, one architecture.**
- In the Effector deployment ZIP, the package is a directory `mindcore/<arch>/` (`x86_64` or `aarch64`) holding `MANIFEST`, `MANIFEST.SIG` and the files `MANIFEST` names.
- The ZIP is transport. What binds the command to it is point 3 of requests-EFF.md: the command names the archive's size and SHA-256, and a ZIP that differs is refused before it is opened.

**The canonical signed bytes are `MANIFEST`'s exact bytes.** It is ASCII lines ending in LF, in this order, with nothing else in the file:
```text
MIND-APP-MANIFEST 1
key <first 16 hex digits of the SHA-256 of the signing public key>
name <1 to 32 of a-z 0-9 ->
version <decimal, 1 to 2^64-1>
arch <x86_64 | aarch64>
system <the lowest release sequence it runs on, decimal>
file <path> <size> <sha256 hex> <requests hex, 8 digits> <memory>
```
- `file` lines follow, one per file, sorted by path. A path is relative, of `a-z 0-9 . _ -` and `/`, with no `..`, at most 128 bytes, and is installed under `data/apps/<name>/<version>/`.
- The request flags and memory are the boot manifest's (`scripts/sign_manifest.py`). They describe what a program asks for and grant nothing (MC-3.11).
- A manifest larger than 64 KiB, with more than 256 files, an unknown line, a field out of range or a line out of order is refused, not repaired.

**The signature.** `MANIFEST.SIG` is 64 bytes: Ed25519 (RFC 8032) over `"MIND application package 1\0"` followed by `MANIFEST`'s bytes. The prefix keeps an application key's signature from ever passing as a boot manifest's or a channel's.

**The key role "application".**
- It is separate from the boot key and the release key: it signs nothing else, and they sign no application (351-UPD-0009's roles).
- The owner says which application keys the device trusts, in a file the network cannot write (the policy file of 750-EFF-0003, changed only with the owner's confirmation, as `netpolicy.txt` is):
  ```text
  application-key <64 hex digits of the public key> <name, or * for any>
  ```
  A key is trusted only for the names on its line. Removing the line revokes it: an installed application signed by it no longer starts.
- A test key derived from public text, as the boot and release test keys are, signs the test packages. It is never on a real device's list.

**Install, update, uninstall: no scripts.** Effector's `manifest.json` entries for `os: "mindcore"` name actions, never a program to run:
- `install`: check the ZIP's size and SHA-256, the signature against a trusted key for that name, `arch` against the machine, `system` against the running release, and every file's size and SHA-256; then write the files under `data/apps/<name>/<version>/`, with `MANIFEST` and `MANIFEST.SIG`, and switch `data/apps/<name>/current` to the version in one rename.
- `update`: an install of a newer version. The previous one stays until the next update, so one step back is a rename.
- `uninstall`: remove `current`, then the version's files. A running instance is stopped first if the policy allows it, otherwise the answer is `denied`.
- A failure leaves the previous `current` as it was. Applications on the boot volume, boot services and boot files are never touched (they change through 750-EFF-0009 and `updater`).

**Rollback.**
- A version lower than the installed one is refused (MC-9.4) unless the owner's policy names it: `application-downgrade <name>`.
- The same version again is accepted only with the same `MANIFEST` bytes.
- What this does not cover: someone with the disk can put back an older `data/apps` tree. The loader's check below refuses it only while its key is untrusted or its version is lower than one the policy records.

**The loader's check (`KRN`'s part, requested when the routine exists).** Before it runs a program from `data/apps/`, the loader checks:
- the stored `MANIFEST.SIG` against the trusted keys;
- the program file's size and SHA-256 against its `file` line;
- that its requests are no more than the line says.

Any mismatch: it does not run, and the log says why.

**Versions of this contract.** The first line's `1` is the format's version. A new field, bound or action is a new version with a stated transition (MC-12.4, 12.7). A device refuses a version it does not know.

## What is built under this task, once the contract is agreed

- **The check, shared:** `libmind::app_package` (`no_std`), used by the gateway and the loader: parse `MANIFEST` with the bounds above, check the signature, list the files.
- **The tool:** `scripts/app_package.py`, beside `scripts/release.py`. It builds `MANIFEST` from a directory, signs it with `$MIND_APP_KEY` or the test key, and checks a package as the device does.
- **Host tests:** the routine accepts the tool's test package. It refuses each of:
  - a changed file, a missing or extra file;
  - a wrong key, a key not trusted for the name;
  - another architecture, a lower version;
  - each bound, and a boot or channel signature presented as an application's.

## Acceptance criteria

- The contract is agreed and published as `docs/update/application-packages.md` 1.0 (EN, RU).
- The host tests above pass.
- The loader's part is recorded in requests-KRN.md, and the gateway's in 750-EFF-0008.
- What this does not show: an install by the gateway, or a check by the loader. Those are 750-EFF-0008 and `KRN`'s task, each with its own tests.

## Related

- [750-EFF-0008](750-EFF-0008-application-packages.md), [750-UPD-0020](750-UPD-0020-the-gateways-client-of-the-updater.md).
- [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) (key roles), [requests-EFF.md](requests-EFF.md).
