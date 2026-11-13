# Update keys: their lifecycle and the compromise protocol

**Version:** 0.1 (2026-11-13) · **Track:** `UPD`, task [351-UPD-0031](../../issues-done/351-UPD-0031-key-lifecycle-and-compromise-protocol.done) · **Constitution:** MC-9.4, 9.5, 9.6, 11.9 · Russian: [keys_RU.md](keys_RU.md)

This document gives each key that vouches for an update:
- its lifecycle: creation, assignment, storage, use, rotation, revocation and destruction (MC-9.6);
- what to do when it is compromised.

The formats and the checks are in [publishing.md](publishing.md) (the key file, channels, timestamps, recovery orders), [README.md](README.md) and [slots.md](slots.md) (the bootloader), and [updater.md](updater.md) (the device). Each procedure names its command and the test that shows the device's side of it.

Tests are evidence, not proofs (MC-12.2). The steps done by people (where a seed is made and kept, when it is destroyed) are procedures this document asks for; no test shows they were followed (MC-12.3).

## The keys

| Role | Signs | Threshold | Held by, stored | Built in |
|---|---|---|---|---|
| `root` | key files (`keys/<version>`): the version after its own | its role's, e.g. 2 of 3 | each key by a different person, offline, on removable media kept apart | the first set, in the bootloader and the updater (`$MIND_ROOT_KEYS`) |
| `release` | channel files (`channels/<name>`) | its role's | the publisher, on the machine that publishes (`$MIND_RELEASE_KEY`) | none |
| `timestamp` | timestamp files (`timestamps/<name>`), renewed at least daily | its role's | the publishing server, which renews them alone (`$MIND_TIMESTAMP_KEY`) | none |
| `boot` | boot manifests (`MANIFEST.SIG`) | 1: a manifest carries one signature | whoever builds a release, on the build machine (`$MIND_BOOT_SIGNING_KEY`) | none: the key file names it, the bootloader takes it from the slot's key file (351-UPD-0029) |
| `recovery` | recovery orders (`recovery/<name>`) | its role's | apart from the release key, offline (`$MIND_RECOVERY_KEY`) | none |
| test keys | the same, for tests | as the test key file says | anyone: derived from public text in `scripts/sign_manifest.py` and `scripts/release.py` | the test root set, when no `$MIND_ROOT_KEYS` is given |
| Secure Boot `PK`, `KEK`, `db` | the bootloader, for the firmware | — | apart, as [secure-boot.md](secure-boot.md) says (351-UPD-0012) | enrolled in the firmware |

- **A key is a seed:** 32 bytes in 64 hex digits, in a file outside the repository. The tools read it from a file named by an environment variable or an argument, never from the command line itself (MC-11.9).
- **One key, one role.** A key in two roles makes one compromise two. `release.py keys make` does not refuse it; whoever makes the key file checks it.
- **The device holds no key that signs updates.** Its own key, `keystore`'s, signs only for the device itself: TLS and SSH ([docs/profile/tcb.md](../profile/tcb.md)). An update is checked with public keys alone.

## The lifecycle

### Creation and assignment

- **Making a seed.** The tools do not make seeds. A seed is made on the machine that will keep it, from that machine's random source, for example `python3 -c "import secrets; print(secrets.token_hex(32))" > /media/root1.seed`. A root key's machine is not on the network while the seed exists.
- **The root set:** `release.py keys root SEED… --threshold T > root.pub` gives the public file. The bootloader and the updater are built with it (`$MIND_ROOT_KEYS`). Test: `tests/release_test.py` (`keys root`); every QEMU suite boots under the test set.
- **The key file:** `release.py keys make FILE --version 1 --role boot=…:1 --role recovery=… --role release=… --role root=…:T --role timestamp=…` assigns each role its public keys and threshold. The file is then signed one root key at a time, on each key's own machine (`keys sign FILE SEED`). It is checked (`keys check FILE --root root.pub`) and published (`keys publish FILE DEST --root root.pub`). Tests:
  - `tests/release_test.py`: make, sign, check, publish, the threshold;
  - `tests/release_host.rs`: the form and the count of signatures;
  - the QEMU `update` suite: a key file only one of two root keys signed is refused, with no record written.

### Storage and use

- **Root keys:** kept offline, each apart from the others. Used only by `keys sign`, on their own machine, for a new key file version.
- **Release key:** used by `release.py publish`, after `stage` has checked the build's manifests.
- **Timestamp key:** on the publishing server, used by `release.py timestamp` from a timer there. It cannot change what a channel says, only vouch that the channel is current ([publishing.md](publishing.md)). Test: the QEMU `update` suite renews a timestamp without the release key.
- **Boot key:** used by `sign_manifest.py` when the build signs a volume. `sign_manifest.py` refuses a real boot key that the key file of `$MIND_KEYS` does not name. Tests: `tests/manifest_test.py`; every QEMU suite boots a signed volume.
- **Recovery key:** used only by `release.py recover`, for an order to go back. Test: the QEMU `update-recovery` suite (an order signed by the release key is refused; one signed by the recovery key is applied once).

### Rotation and revocation

A key is replaced or withdrawn by the key file's next version, signed by the threshold of the current root and of its own ([publishing.md](publishing.md), "Rotating and revoking a key"). The two commands write that version's body:
- `release.py keys rotate FILE ROLE OUT --key K… --threshold T`;
- `release.py keys revoke FILE KEY OUT`, which refuses to leave a role short of its threshold.

The body is then signed and published as above. What each device does:

| Role | What changes on the device | Test |
|---|---|---|
| `root` | The updater takes the next version only if the old root and the new one each signed it to their thresholds. It then trusts the new root alone, and keeps that version in `system/updater/roots`. The bootloader follows the change through the slot's `KEYS.1`, `KEYS.2`, … | the QEMU `update-keys` suite (version 3 rotates the root; a version 4 the old root signed is refused); `tests/boot_keys_host.rs` |
| `release` | A channel only the old key signed is refused. So is a slot whose kept channel only it signed, which is then no fallback. | the QEMU `update-keys` suite |
| `timestamp` | A timestamp only the old key signed is refused, as one signed by any key the role does not name, and so is the channel it vouches for. | `tests/release_host.rs`; the QEMU `update` suite (a timestamp the release key signed is refused) |
| `boot` | The updater stages only a manifest a key of the new `boot` role signed, with the new key file. Once that slot is confirmed, the records name the key file's version, and the bootloader refuses a slot whose key file is older or whose manifest the key file's boot keys did not sign. | the QEMU `updatebootkey` suite (351-UPD-0029) |
| `recovery` | An order only the old key signed is refused, as one signed by any key the role does not name. | the QEMU `update-recovery` suite (an order the release key signed is refused); `tests/release_host.rs` |

- **After a rotation, publish under the new key at once.** A new channel signed by the new release key; for a boot key, a release signed by the new key. A device keeps what it runs until then.
- **A lower key file served again is refused** by every device that took a higher one. Test: the QEMU `update-keys` suite.
- **A key file expires** (`--days`, a year by default). A device past the expiry holds what it has, as for a channel ([updater.md](updater.md), "Metadata past its expiry"). Renew it with a new version before it expires.

### Destruction

- **A seed is destroyed** when its key is rotated away and the new version has been published: the file and every copy of it are overwritten and removed (`shred -u`), and a root key's medium with them. A destroyed key can no longer be abused, and nothing a device needs is lost: a device needs public keys only.
- **Kept, not destroyed:** the public key files (`root.pub`, the key history `keys/<version>`). A device walks the history from version 1, and `release.py check` and `bundle` need it.
- **No test** shows a seed was destroyed; it is a procedure (MC-12.3).

## The compromise protocol

Whoever holds a compromised key can sign what that key signs, until devices take a key file without it. The steps are the same for each role:
1. publish the key file's next version without the key (`keys revoke`, or `keys rotate` with a new key);
2. publish under the new key what that role signs;
3. destroy the old seed.

What differs is what the key could do meanwhile, and what is left after it.

| Role | What its holder can do | What stays valid | Recovery |
|---|---|---|---|
| `release` | Sign a channel naming any release the server holds: offer an older one at or above the floor, or hold devices back. It cannot make a release: each manifest needs the boot key, and each channel a current timestamp. | Every release already published; a device's running release and its fallback, whose channel a key the new file names signed | Unaffected: the recovery key is a separate role |
| `timestamp` | Keep a channel current past its time: a device sees no newer channel (a freeze). It cannot change what the channel names. | Every channel and release | Unaffected |
| `boot` | Sign a manifest for a build of its own. A device fetches it only if a channel signed by the release keys names it. Someone who can write the disk can put it in a slot. | Releases signed by other boot keys the new file names. Once a release under the new key file is confirmed, the bootloader refuses a slot whose key file is older. | Unaffected; a recovery order needs the release signed by a current boot key in the other slot |
| `recovery` | Send devices back to the release their other slot holds, with the floor lowered, once per order sequence. | Every release; the floor rises again with the next confirmed minimum | Itself: rotate it with the root's threshold |
| `root`, below its threshold | Nothing alone. | Everything | Rotate the root before more keys are lost |
| `root`, at its threshold | Sign the next key file: every role, the root included. This is the whole update chain. | Nothing it signs can be told from the owner's, from that version on | New bootloader and updater, built with a new root set, written to each device by hand; a device that took a key file the attacker signed takes none of the owner's |
| test keys | Anything, for anyone: they are public. | Nothing of a release: a build with them says `THE TEST KEY` | None needed: no release is made with them |
| Secure Boot | Sign a bootloader the firmware runs | As [secure-boot.md](secure-boot.md) says: an old bootloader is revoked through `dbx` | — |

- **How devices learn of it:** only from the key file, at their next check (`automatic check`, `update check`, or a bundle through the Effector gateway). There is no other revocation list.
- **An offline device** learns nothing until it reaches a source. Meanwhile its metadata expires, and past the expiry it holds the release it runs, stages nothing and applies nothing (351-UPD-0025, [updater.md](updater.md)). A short timestamp limits how long a channel can be shown current to a device that is online.
- **Recovery stays available** while the recovery role is not compromised. It is separate from the release, timestamp and boot keys, and a recovery order needs no release key. It goes only back to a release the other slot holds, which its boot key and key file must still pass.
- **What a key change does not undo** (MC-9.6, last sentence):
  - a release a compromised key let a device install stays installed until a newer one replaces it;
  - what that release did while it ran stays done: data it read, keys it used, messages it sent;
  - a release that changed the updater itself may refuse what comes next: such a device is written again by hand;
  - a rotation stops further acceptance, nothing more.

## MC-11.9: the key and the right to use it

- **Signing runs through the tools** (`release.py`, `sign_manifest.py`). They read a seed from a file named by an environment variable or an argument, never from the command line.
  - The root keys are used only by `keys sign`, one at a time, so that no single machine holds the threshold.
  - The timestamp key is the only one on the publishing server, and it signs only timestamps.
- **A device holds no key that signs updates.** `keystore`'s device key signs for TLS and SSH; the `updater` and the bootloader hold public keys only.
- **Not provided:** a hardware token for the root, release or boot keys. The tools read seeds from files, so a machine that holds a seed can sign anything that key signs. The right to request a signature is not separate from the key there.

## Checked against the Constitution

| Clause | What this document and the code give | What is not given |
|---|---|---|
| MC-9.4: no unauthorized rollback; a permitted one respects the minimum version and separate recovery authorities; expiry by a set policy | The floor in the boot records, raised to a channel's minimum (351-UPD-0024); lowered only by a recovery order the recovery role signed (351-UPD-0030); a slot below the records' key file version refused (351-UPD-0029); metadata past its expiry handled as [updater.md](updater.md) says (351-UPD-0025) | A floor someone with the disk cannot lower (351-UPD-0011, TPM) |
| MC-9.5: keys, algorithms, formats replaceable without silently dropping verification | Every role rotated through the key file, the root too, at boot too (351-UPD-0027, 0029); records of an older format and slots without a key file are refused, not read as unverified | A second signature algorithm: Ed25519 alone |
| MC-9.6: the lifecycle, the compromise protocol, separation, what a key change does not undo | The lifecycle and protocol above; five roles, each with its own keys and threshold; recovery separate from release | No test shows the people's steps (where seeds are made and kept, when destroyed) were followed |
| MC-11.9: key material distinct from the right to request a signature | On the device: public keys only, and `keystore`'s key apart from the update chain. Off it: the tools and the separation of roles above | A hardware token or signing service for the update keys |

## Not provided yet

- A hardware token, or a signing service apart from the machine that asks, for the root, release and boot keys (MC-11.9).
- A tool that makes seeds, or that checks one key is not in two roles.
- A root compromised at its threshold leaves only a new image written by hand.
