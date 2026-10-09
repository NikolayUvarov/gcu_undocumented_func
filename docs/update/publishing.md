# Publishing releases

**Version:** 0.1 (2026-10-08) · **Track:** `UPD`, task [351-UPD-0005](../../issues-done/351-UPD-0005-release-and-publish.done) · **Constitution:** MC-9.2, 9.4, 9.6, 9.7 · Russian: [publishing_RU.md](publishing_RU.md)

How a build becomes a release a device can fetch and check, how to publish it to a server, and who holds which key. The device side, the updater, is not built yet ([351-UPD-0007](../../issues/351-UPD-0007-updater-service.md)). Until it is, a release is checked on the host only.

## What a release is

A release has:
- a **version**: an integer, never reused and never lowered;
- for each architecture, the boot **manifest** and its signature as the build made them ([README.md](README.md));
- every file those manifests list, stored once by its SHA-256.

A **channel** (`stable`, `test`, …) names the latest version, the **minimum** version a device may run, an **expiry**, and the SHA-256 of each architecture's manifest. It is one line of JSON in one encoding (sorted keys, no spaces), then `ed25519 <signature in hex>` over that line, signed with the **release key**. Both are in one file, so replacing it with one rename changes them together.

Server layout:

```
blobs/<sha256>                              every file of every release, by content
releases/<version>/<arch>/MANIFEST          the boot manifest of that architecture
releases/<version>/<arch>/MANIFEST.SIG      its signature (the boot key)
channels/<channel>                          the signed channel file (the release key)
```

## Making and publishing a release

```bash
./02_build.sh && ARCH=aarch64 ./02_build.sh                # builds and signs usb_root/ and aarch64_root/
python3 scripts/release.py stage 7 /tmp/release-7          # checks each volume, then copies it as version 7
python3 scripts/release.py publish /tmp/release-7 updates@example.org:/srv/mind --channel stable --minimum 5
python3 scripts/release.py check /srv/mind --channel stable   # on the server, or on a copy
```

- `stage` refuses a volume whose manifest does not verify. Releases are made from signed builds only.
- `publish` refuses a version that is not above the channel's, and a version whose manifests are already on the server. It uploads in this order:
  1. the blobs, never overwriting one;
  2. the release's manifests;
  3. the channel, in one rename.

  An upload cut at any point leaves the previous channel naming files that are all there.
- The destination is a directory on this machine or `host:directory`, reached with the system's OpenSSH (`ssh`, `rsync -e ssh`). The SSH login belongs to the uploader and is never the release key.
- `check` verifies:
  - the channel's signature, encoding and expiry;
  - every manifest it names: its hash and its boot signature;
  - every blob those manifests list: its size and hash.

## Keys and who holds them (MC-9.6)

| Key | Signs | Held by | In this repository |
|---|---|---|---|
| Boot key | boot manifests (`MANIFEST.SIG`) | whoever builds a release; its public half is built into the bootloader | only the **test** key, derived from public text (`scripts/sign_manifest.py`) |
| Release key | channel files | whoever publishes | only the **test** key (`scripts/release.py`) |
| SSH login | nothing: it lets the uploader write the server | the uploader's own SSH key | none |

- A real key is a file of 64 hex digits (a 32-byte seed) outside the repository. It is named by `$MIND_BOOT_SIGNING_KEY` and `$MIND_RELEASE_KEY`, and the bootloader is built with the boot key's public half (`$MIND_BOOT_PUBLIC_KEY`).
- The test keys are public. A release signed with them is checked for accidents, not against an attacker, and the tools say `THE TEST KEY` / `THE TEST RELEASE KEY`.
- Rotation, revocation and the compromise protocol are not defined yet ([351-UPD-0009](../../issues/351-UPD-0009-rollback-policy-and-key-roles.md)).

## Setting up a server

Any static web server over the directory serves a channel over HTTPS. The SSH account that publishes needs only write access to that directory. For tests, `scripts/serve_release.py DIR --cert … --key …` serves a directory over TLS 1.3 with a certificate the test gives.

## Not provided yet

- the updater on the device (351-UPD-0007) and its HTTPS and SSH clients (351-NET-0002, 351-NET-0004; plain HTTP downloads with resume are 351-NET-0001);
- a check of the channel's minimum and expiry by a device (351-UPD-0009);
- an SSH test server in CI (OpenSSH's server is not on the runners; the SSH path of `publish` is not tested).
