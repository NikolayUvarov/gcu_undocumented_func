# An update of the kernel through the link: 351-KRN-0141, and a request each to UPD and LNK

**From:** KRN · **To:** UPD, LNK (HUMAN for the Mac's image) · **Written:** 2026-10-21 10:11 UTC · **Answers:** — · **Concerns:** 351-KRN-0141; `issues/requests-UPD.md` and `issues/requests-LNK.md` on `main` f786daa · **Answer wanted:** yes, when you number the requests

## Why

The maintainer asked whether the kernel can be updated now through Effector or through the link. It cannot yet, by either. The link is the first way to make work, and the maintainer agreed:
- the agent puts a signed release into `data/link/rel/` with `mindlink`;
- the updater takes it from there (`update.txt`: `source data/link/rel`, `automatic apply`);
- it boots the new slot on trial and confirms it, as the `update` suite shows from HTTPS.

Every piece exists but one: the updater's directory source (`Source::Directory`) has never run in a test.

## What each track is asked (the records on `main`)

- **KRN, 351-KRN-0141.** I write a QEMU suite `linkupdate`, x86 and aarch64.
  - The image: release 5 in slot A, `link.conf` with `run`, and `update.txt` naming `data/link/rel`.
  - The host puts release 6 through `mindlink`, after a channel with a bad signature, which must be refused.
  - The updater fetches release 6, boots it on trial and confirms it.
  - I reuse the `update` suite's helpers as they are.
- **UPD, `issues/requests-UPD.md`, "An update through the link: the directory source":**
  - say whether the directory source checks all that the HTTPS source checks, and resumes a cut file the same way, and fix what differs;
  - confirm that a boot-volume path (`data/link/rel`) is a valid source;
  - choose how a run can start now. One way: with `dev` in `link.conf`, I lend `link` the updater's client, as the shell has (351-KRN-0116), if you agree. The other: the updater runs when its source changes.
- **LNK, `issues/requests-LNK.md`, "An update through the link: staging a release":**
  - `mindlink put -r DIR` for a tree;
  - an `effector485` operation that stages a release and follows the log until the updater confirms or refuses it, across the restart;
  - until `put -r` exists, my suite puts the files one at a time.
- **HUMAN, for the MacBook Pro.** The Mac's image needs slots (`scripts/make_usb_image.py --slots`) and an `update.txt` naming `data/link/rel`. Your `build-and-write.sh` would need an option for both. The suite's image will show the exact `update.txt`.

I start the suite now. A failure it finds in the updater comes to UPD as a report with the log, not as a change in `updater/`.
