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

## Progress (2026-10-21)

**The `linkupdate` suite is written** (`tests/qemu_smoke.py`, in the group "x86: files and block writes"; on aarch64 it joins "aarch64: files, network and TLS" once the agent track's `effector` suite, added to the same line, is in `main`).
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

- **Criterion 1 on x86:** the suite passed in landing round 3's gate (`land-e4fd3b37`, "x86: files and block writes", every group green), `main` e4fd3b3.
- **On aarch64:** it joins "aarch64: files, network and TLS" in round 4, on top of EFF's `effector` suite in the same line. Locally it passed twice more with the script's code, x86 in 174 s and aarch64 in 274 s through the link.
- **For criterion 2:** the suite shows that the directory source refuses a bad signature and that its release passes the manifest's digests, through the same `stage()` as HTTPS. Whether it checks all that HTTPS checks (an expired or older channel, a changed blob, a resumed file) is still the update track's to say.

**For criterion 3: `scripts/link_update.py`** (2026-10-21). The maintainer said the MacBook Pro now restarts into MIND Core by itself.
- **What it does, in one command on the machine at the other end of the cable:**
  - it makes the current build a signed release, as `scripts/release.py` does. The version defaults to the time (`YYYYMMDDHHMM`), so it is above any earlier one;
  - it puts only what the updater reads into `data/link/rel/`, the channel last. `mindlink put` skips a blob the Mac holds already, so a second update sends only what changed;
  - it follows `mindlink log` through the trial restart. It ends at `[UPDATER] SLOT … CONFIRMED ON THE DISK` (exit 0), or at a refusal, a rollback or the time limit (exit 1).
- **The `linkupdate` suite now runs the script's own code:** what is put, the puts, and the follower that ends at the refusal and at the confirmation. `tests/link_update_test.py` checks the follower on scripted logs.
- **The maintainer's run, once:**
  1. Build: `./02_build.sh`.
  2. Write the stick with slots and this `update.txt` (`make_usb_image.py --slots --update update.txt`, plus `link.conf` as `build-and-write.sh` writes it now):

     ```text
     source data/link/rel
     channel stable
     automatic apply
     every 20
     ```

  3. Boot the Mac from it. `mindlink log` should show `[UPDATER] SLOT A, VERSION 0 …` and checks refused as `Missing` every 20 s.
- **Every update after that:**
  - build;
  - `python3 scripts/link_update.py COM<n> --mindlink path\to\mindlink.exe --mindlink-option=--baud --mindlink-option=460800`, with the options the cable needs.
- **Not done yet:** the Mac's run itself, recorded here with its configuration when it is made. The release LNK's `effector485 stage` makes (351-LNK-0013) can use the same layout.

## Related

[351](351-self-update.md), [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done) (the updater), [351-UPD-0016](../issues-done/351-UPD-0016-images-with-slots.done) (images with slots), [800-LNK-0010](800-LNK-0010-directories-in-data-link.md) (directories over the link), [800-LNK-0011](800-LNK-0011-effector485.md) (`effector485`).
