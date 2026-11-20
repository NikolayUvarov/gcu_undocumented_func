# 352-UPD-0050 — A release's artifacts from one commit, with one signed list of their sums

**Type:** script, release · **Owner:** `UPD` (the kernel session) · **Priority:** P2 · **Status:** open (plan, 2026-11-20) · **Blocked by:** [352-UPD-0047](352-UPD-0047-an-installer-iso.md), [352-UPD-0049](352-UPD-0049-images-for-arm-boards-and-phones.md) (their artifacts) · **Main task:** [352](352-installable-media-for-pcs-and-arm-devices.md) · **Constitution:** MC-9.1, MC-9.2, MC-9.5, MC-9.7, MC-12.1, MC-12.3

## Problem

`scripts/release.py` stages a release, signs its channel and checks it. `bundle` makes one ZIP for the updater of one architecture ([docs/update/publishing.md](../docs/update/publishing.md)).

**What a person who downloads MIND Core cannot do today:**
- **find every medium of one version in one place.** The USB images and the boards' cards are built by separate commands, from local builds;
- **check what was downloaded** with one signed list: a channel's signature covers the updater's files, not an ISO or a card image;
- **know that two artifacts carry the same system:** nothing says that the ISO, the USB image, a board's card and the update package of one version hold the same boot volume.

## Plan

1. **One command,** `release.py artifacts --version V --out DIR`, from a clean tree at one commit:
   - for x86-64 and aarch64:
     - the ISO (352-UPD-0047);
     - the raw USB image (`make_usb_image.py --slots`);
     - the update package (`bundle`);
   - for each device of 352-UPD-0049's catalog: its image, made where the firmware is at hand. The firmware is fetched on the maintainer's machine, so the CI builds the others and says which images it left out.
2. **Every artifact of an architecture holds the same boot volume:** one FAT image, built once and checked against the signed manifest, then put into the ISO, the USB image and the cards. The list says so (MC-9.1).
3. **`SHA256SUMS`:**
   - one line per artifact: its SHA-256, its size and its name;
   - signed by the release key, as the channel is, with the signature in `SHA256SUMS.sig`. It is checked with `release.py verify-sums` or with the public key and a standard tool (the docs say which);
   - each line also names the commit, the toolchain's versions and the firmware's sources (MC-9.7).
4. **Reproducible where the build is.** `350-UPD-0001`'s check is extended to the ISO, the USB image and the bundle:
   - a second build of the same commit gives the same sums;
   - what differs is named, and why (MC-9.5).
5. **Publishing stays the maintainer's act:**
   - signing with the real release key, on the maintainer's machine;
   - the upload to the release server or to MIND Hub ([100003](100003-mind-hub-requests-conversation-check-and-updates.md));
   - a GitHub release, if one is wanted (issues-human).
   - The CI builds the same artifacts under the test keys, so the steps are checked on every commit.
6. **The docs:** docs/update/publishing.md (EN and RU) says how a release is made. docs/write-disk.md says which artifact to take for which machine, and how to check it.

## Acceptance criteria

- [ ] `release.py artifacts` makes the ISOs, the USB images and the update packages of both architectures, and every board image whose firmware is present, with `SHA256SUMS` and its signature (host test, test keys).
- [ ] The same commit built twice gives the same `SHA256SUMS`, except the lines the reproducibility check names.
- [ ] A changed byte in any artifact, or in `SHA256SUMS`, is refused by `verify-sums`.
- [ ] The ISO, the USB image and a board image of one architecture hold the same boot volume (the same SHA-256 of its FAT image).
- [ ] A CI job builds the artifacts under the test keys and keeps them for a short time; it is not a release.

## Related

[352](352-installable-media-for-pcs-and-arm-devices.md), [350-UPD-0001](../issues-done/350-UPD-0001-reproducible-build-check.done), [351](351-self-update.md), [docs/update/publishing.md](../docs/update/publishing.md), [docs/update/keys.md](../docs/update/keys.md), [100003](100003-mind-hub-requests-conversation-check-and-updates.md) (MIND Hub, the release server).
