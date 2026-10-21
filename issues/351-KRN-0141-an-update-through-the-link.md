# 351-KRN-0141 — An update through the link: a release put into `data/link/` installed by the updater

**Type:** test (QEMU), coordination · **Owner:** kernel session · **Priority:** P1 · **Status:** open (2026-10-21) · **Blocked by:** — for the suite; the requests in [requests-UPD.md](requests-UPD.md) and [requests-LNK.md](requests-LNK.md) ("An update through the link") for the rest · **Main task:** [351](351-self-update.md) (the update track's) · **Constitution:** MC-9.2–9.4, MC-12.1, MC-12.9

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
3. **The link track** ([requests-LNK.md](requests-LNK.md)):
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

## Progress (2026-10-21)

**The `linkupdate` suite is written** (`tests/qemu_smoke.py`, in the groups "x86: files and block writes" and "aarch64: files, network and TLS").
- **The image:** the update suite's, with `link.conf` (`baud=115200`, no `dev`) and `update.txt` naming `data/link/rel`, no network card. The release building moved into `update_releases()`, shared with the update suite.
- **What goes through the link:** only what the updater reads from its source. That is the boot files' blobs, `releases/6/<arch>/MANIFEST` and `MANIFEST.SIG`, then `channels/stable`. Each is put by its own `mindlink mkdir` and `put`.
- **What it checks:**
  - a channel whose signature was changed is refused (`CHECK: REFUSED: Signature`, read with `mindlink log`), and nothing is fetched or recorded;
  - the channel as published is fetched from the directory into slot B and booted on trial; init confirms it;
  - the updater confirms slot B on the disk, read again with `mindlink log` after the restart;
  - the records, slot B's files and the file system are as the update suite finds them;
  - without `dev`, `link` holds none of the developer mode's grants (for 800-KRN-0139).
- **Local runs, this machine's QEMU, 4 CPUs, two each, all passed:**

  | Architecture | Files | Bytes | Time to put them through the link |
  |---|---|---|---|
  | x86 | 43 | 9 643 243 | 185 s and 186 s (about 52 KB/s) |
  | aarch64 | 37 | 8 590 656 | 294 s and 289 s |

- **Left for criterion 1:** the gate. The suite is on `claude/youthful-mendel-mf1soy` 7361af5 and goes to `main` through the landing queue.
- **For criterion 2:** the suite shows that the directory source refuses a bad signature and that its release passes the manifest's digests, through the same `stage()` as HTTPS. Whether it checks all that HTTPS checks (an expired or older channel, a changed blob, a resumed file) is still the update track's to say.

## Related

[351](351-self-update.md), [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done) (the updater), [351-UPD-0016](../issues-done/351-UPD-0016-images-with-slots.done) (images with slots), [800-LNK-0010](800-LNK-0010-directories-in-data-link.md) (directories over the link), [800-LNK-0011](800-LNK-0011-effector485.md) (`effector485`).
