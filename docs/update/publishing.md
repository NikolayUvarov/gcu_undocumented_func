# Publishing releases

**Version:** 0.4 (2026-11-13): the key file and the root keys, and signature lines that name their keys (351-UPD-0026); 0.3 (2026-10-10): the device side is built: the updater (351-UPD-0007); 0.2: `check` reads a channel's fields before trusting them (351-UPD-0014); 0.1 (2026-10-08) · **Track:** `UPD`, task [351-UPD-0005](../../issues-done/351-UPD-0005-release-and-publish.done) · **Constitution:** MC-9.2, 9.4, 9.6, 9.7 · Russian: [publishing_RU.md](publishing_RU.md)

How a build becomes a release a device can fetch and check, how to publish it to a server, and who holds which key. The device side is the updater ([updater.md](updater.md)): it checks a channel and stages its release in a slot with the same checks as `check` below.

## What a release is

A release has:
- a **version**: an integer, never reused and never lowered;
- for each architecture, the boot **manifest** and its signature as the build made them ([README.md](README.md));
- every file those manifests list, stored once by its SHA-256.

A **channel** (`stable`, `test`, …) names the latest version, the **minimum** version a device may run, an **expiry**, and the SHA-256 of each architecture's manifest. It is one line of JSON in one encoding (sorted keys, no spaces), then one to eight signature lines over that line, `ed25519 <public key> <signature>` in hex, by the **release keys** the key file names, at least as many as its threshold. Both are in one file, so replacing it with one rename changes them together.

The **key file** (351-UPD-0026, after TUF's root metadata) says which keys are current. It is one line of JSON in one encoding: its `version`, its `expires`, and for each role, `boot`, `recovery`, `release` and `timestamp`, one to four public keys in 64 lower-case hex digits in ascending order and a `threshold` from 1 to their number. Signature lines of the same form follow, by the **root keys**. A device takes it when at least the root threshold of the root keys built into it signed it: each root key counts once, however many lines name it, and a line of any other key is passed over. A channel of before this format, its one line `ed25519 <signature>` naming no key, is refused (MC-12.4). Today the updater uses the release role; the boot, recovery and timestamp roles are named for [351-UPD-0029](../../issues/351-UPD-0029-the-boot-key-under-the-root.md), [351-UPD-0030](../../issues/351-UPD-0030-a-separate-recovery-authority.md) and [351-UPD-0028](../../issues/351-UPD-0028-a-short-lived-timestamp-key.md).

```
{"expires":"2027-11-13T00:00:00Z","roles":{"boot":{"keys":["662f…"],"threshold":1},"recovery":{…},"release":{"keys":["98d6…"],"threshold":1},"timestamp":{…}},"version":1}
ed25519 <root key 1> <its signature of the first line>
ed25519 <root key 2> <its signature of the first line>
```

Server layout:

```
blobs/<sha256>                              every file of every release, by content
releases/<version>/<arch>/MANIFEST          the boot manifest of that architecture
releases/<version>/<arch>/MANIFEST.SIG      its signature (the boot key)
channels/<channel>                          the signed channel file (the release keys)
keys/current                                the key file (the root keys)
```

## Making and publishing a release

```bash
./02_build.sh && ARCH=aarch64 ./02_build.sh                # builds and signs usb_root/ and aarch64_root/
python3 scripts/release.py stage 7 /tmp/release-7          # checks each volume, then copies it as version 7
python3 scripts/release.py publish /tmp/release-7 updates@example.org:/srv/mind --channel stable --minimum 5
python3 scripts/release.py check /srv/mind --channel stable   # on the server, or on a copy
python3 scripts/release.py bundle /srv/mind /tmp/mind-7.zip --arch x86_64   # one file for the Effector gateway
```

A key file is made once and again when a key changes, signed one root key at a time on the machines that keep them:

```bash
python3 scripts/release.py keys root /media/root1.seed /media/root2.seed /media/root3.seed --threshold 2 > root.pub   # $MIND_ROOT_KEYS's file
python3 scripts/release.py keys make keys-1 --version 1 --days 365 --role boot=<key>:1 --role recovery=<key>:1 --role release=<key>:1 --role timestamp=<key>:1
python3 scripts/release.py keys sign keys-1 /media/root1.seed                 # on the first root key's machine
python3 scripts/release.py keys sign keys-1 /media/root3.seed                 # on another's
python3 scripts/release.py keys check keys-1 --root root.pub
python3 scripts/release.py keys publish keys-1 updates@example.org:/srv/mind --root root.pub
```

- `keys make` writes the body alone. `keys sign` adds one line and refuses a key that signed already. `keys check` says how many root keys signed it, and its expiry.
- `keys publish` checks it, then writes `keys/current`; a version not above the server's is refused (the same file again is no change).

- `stage` refuses a volume whose manifest does not verify. Releases are made from signed builds only.
- `publish` refuses a version that is not above the channel's, a version whose manifests are already on the server, and release keys the server's key file does not name or fewer than its release threshold. A server without a key file gets the test one when the channel is signed with the test release key alone; with any other key it is refused. It uploads in this order:
  1. the blobs, never overwriting one;
  2. the release's manifests;
  3. the channel, in one rename.

  An upload cut at any point leaves the previous channel naming files that are all there.
- The destination is a directory on this machine or `host:directory`, reached with the system's OpenSSH (`ssh`, `rsync -e ssh`). The SSH login belongs to the uploader and is never the release key.
- `check` verifies:
  - the key file: the root threshold of `$MIND_ROOT_KEYS`'s set (the test set without it), its form and its expiry;
  - the channel's signatures: at least the release threshold of the keys the key file names; then its fields as the device reads them (UTF-8 JSON of `channel`, `version`, `minimum`, `expires` and `manifests`, each of its type and range: 1 ≤ minimum ≤ version < 2^64, `expires` a real time as `YYYY-MM-DDTHH:MM:SSZ`, 1 to 4 architectures with 64 lower-case hex digits each), its one encoding and its expiry. A body signed with the release key that is not a channel gets a reason, never an exception (351-UPD-0014);
  - every manifest it names: its hash and its boot signature;
  - every blob those manifests list: its size and hash.
- `bundle` runs `check` first, then writes one ZIP of stored entries with fixed dates (751-UPD-0032):
  - the key file;
  - the channel file;
  - the manifest and signature of each architecture asked for (all the channel names, by default);
  - the blobs those manifests list.

  The Effector gateway streams it to the updater (`update.wit` 1.3 `install-…`), which checks it as any source ([updater.md](updater.md)). The same release makes the same file.

## Keys and who holds them (MC-9.6)

| Key | Signs | Held by | In this repository |
|---|---|---|---|
| Boot key | boot manifests (`MANIFEST.SIG`) | whoever builds a release; its public half is built into the bootloader | only the **test** key, derived from public text (`scripts/sign_manifest.py`) |
| Root keys | key files (`keys/current`), at least the root threshold of them | each one apart from the others, offline; their public halves and the threshold are built into the updater | only the **test** set: three keys from fixed text, two of which sign (`scripts/release.py`) |
| Release keys | channel files, at least the release threshold of them | whoever publishes; the key file names them | only the **test** key (`scripts/release.py`) |
| SSH login | nothing: it lets the uploader write the server | the uploader's own SSH key | none |

- A real key is a file of 64 hex digits (a 32-byte seed) outside the repository. It is named by `$MIND_BOOT_SIGNING_KEY` and `$MIND_RELEASE_KEY` (several release keys separated by the path separator); a root key is given to `keys sign` alone. The bootloader is built with the boot key's public half (`$MIND_BOOT_PUBLIC_KEY`). The updater is built with it and with the root set's public file, `$MIND_ROOT_KEYS` (`threshold T`, then a key a line, as `keys root` prints it); no release key is built in, the key file names them (`release.py --public` prints the test release key's).
- The test keys are public. A release signed with them is checked for accidents, not against an attacker, and the tools say `THE TEST KEY` / `THE TEST RELEASE KEY` / `THE TEST ROOT KEYS`.
- Rotation, revocation and the compromise protocol are not defined yet ([351-UPD-0027](../../issues/351-UPD-0027-key-rotation-and-revocation.md), [351-UPD-0031](../../issues/351-UPD-0031-key-lifecycle-and-compromise-protocol.md)). A device does not yet refuse a key file of a lower version than one it took before: that is 351-UPD-0027's.

## Setting up a server

Any static web server over the directory serves a channel over HTTPS. The project's own server, `httpd` ([server.md](server.md)), is stage 1 of main task [110](../../issues/110-our-own-web-server.md): a host program for Linux, TLS 1.3 with a pinned key; a MIND Core service is stage 2. The SSH account that publishes needs only write access to that directory. For tests, `scripts/serve_release.py DIR --cert … --key …` serves a directory over TLS 1.3 with a certificate the test gives.

## Not provided yet

- the updater's SSH client (351-NET-0004);
- a policy for the channel's minimum on the device beyond reporting it, and key rotation (351-UPD-0027);
- an SSH test server in CI (OpenSSH's server is not on the runners; the SSH path of `publish` is not tested).
