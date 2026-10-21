# 351-KRN-0141 — An update through the link: a release put into `data/link/` installed by the updater

**Type:** test (QEMU), coordination · **Owner:** kernel session · **Priority:** P1 · **Status:** open (2026-10-21) · **Blocked by:** — for the suite; the request in [requests-UPD.md](requests-UPD.md) ("An update through the link") and [351-LNK-0013](351-LNK-0013-staging-a-release-through-the-link.md) for the rest · **Main task:** [351](351-self-update.md) (the update track's) · **Constitution:** MC-9.2–9.4, MC-12.1, MC-12.9

## Problem

On 2026-10-21 the maintainer asked whether the kernel can be updated now through Effector or through the link, and agreed to make the link the first way.
- **What exists.** The updater stages a signed release into the slot that did not boot and boots it on trial: [docs/update/updater.md](../docs/update/updater.md). The `update` suite shows that on x86 and aarch64 in every gate.
- **The suite's source is a test HTTPS server.** The other source, a directory on a mounted volume (`Source::Directory`, `updater/src/source.rs`), is in the code but no test runs it.
- **The link reaches `data/link/` only.** A release put there through `mindlink` is readable by the updater's file client, and `update.txt` can name that directory (`source data/link/rel`).
- **What is missing:**
  - nothing has put a release there and seen the updater take it;
  - the image `update.txt` belongs to (the boot volume's root, written with the image) has no tool that writes it with such a source;
  - the agent at the other end of the cable cannot ask the updater to check now, so it waits for the next automatic run.

## Plan

1. **The kernel track: a QEMU suite `linkupdate`, x86 and aarch64.**
   - **The image:**
     - release 5 in slot A, confirmed, with its channel kept there, as the `update` suite makes it;
     - `link.conf` with `run`;
     - `update.txt`: `source data/link/rel`, `channel stable`, `automatic apply`, `every 20`.
   - **The host's part:**
     - it publishes release 6 with `scripts/release.py`, signed by the test keys;
     - it puts that release's tree into `data/link/rel/` through `mindlink`, one `mkdir` and `put` at a time.
   - **What the suite checks:**
     - the updater fetches release 6 from that directory, checks it as from HTTPS, and boots slot B on trial;
     - it confirms slot B on the disk;
     - `mindlink log` shows each step;
     - a release whose channel has a bad signature, put first, is refused, with no boot record written.
   - **The update suite's own helpers** (the release, the keys, the slot image) are reused as they are.
2. **The update track** ([requests-UPD.md](requests-UPD.md)):
   - the directory source's checks equal to the HTTPS source's, and a fix for whatever the suite finds;
   - a way for a holder of the updater's client to start a run now.
3. **The link track** ([351-LNK-0013](351-LNK-0013-staging-a-release-through-the-link.md)):
   - `mindlink put -r` of a directory tree in one command;
   - in `effector485`, an operation that stages a release and reports the updater's state from the log.
4. **The maintainer's image for the Mac.** The image needs slots and an `update.txt` naming `data/link/rel`.
   - `scripts/make_usb_image.py --slots` makes the slots.
   - The suite's image shows `update.txt`'s content.
   - The maintainer's `build-and-write.sh` would need an option for both.

## Acceptance criteria

- [ ] The `linkupdate` suite passes on x86 and aarch64, in the gate: release 6 put through the link boots on trial and is confirmed; a channel with a bad signature is refused first.
- [ ] The directory source's checks are the HTTPS source's, said by the update track (or fixed).
- [ ] On the maintainer's MacBook Pro, a release put over RS-485 is installed and confirmed (the maintainer's run, recorded here with its configuration).

## Related

[351](351-self-update.md), [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done) (the updater), [351-UPD-0016](../issues-done/351-UPD-0016-images-with-slots.done) (images with slots), [800-LNK-0010](800-LNK-0010-directories-in-data-link.md) (directories over the link), [800-LNK-0011](800-LNK-0011-effector485.md) (`effector485`).
