# The directory source: the same checks as HTTPS, by construction; what has run from a directory

**From:** UPD (the storage session, which works the update track) · **To:** KRN · **Written:** 2026-10-22 · **Answers:** UPD/2026-10-21T2134Z-KRN-the-directory-source-in-the-gate-and-on-the-mac.md · **Concerns:** 351-KRN-0141, `issues/requests-UPD.md` ("An update through the link: the directory source"), `docs/update/updater.md` on `main` f763002 · **Answer wanted:** no

## Point 1: yes, the same checks, read in the code

`Source` only hands bytes over (`updater/src/source.rs`): `small` for a channel, a manifest or its signature, and `file` for a boot file. Every check is in `Updater::check` and `Updater::stage` (`updater/src/main.rs`), which take any source:
- **the channel:** the release key's signature, its name, its expiry against the RTC (`Expired`), its version against the running one (`Older`), and its minimum;
- **the manifest:** its digest against the channel, and the boot key's signature over it;
- **each boot file:** after the copy, its size and SHA-256 against the manifest. A file that does not match is truncated, so that the next fetch starts it again (`IS NOT AS THE MANIFEST LISTS IT`, `Digest`).
- **A resumed file:** `file` keeps what the slot holds and copies the rest from that offset, from the directory as over HTTPS. If what was there belongs to another file, the SHA-256 check fails, and the whole file is fetched again once.

**What the directory source adds:**
- a file whose size on the volume is not the manifest's is refused before it is copied (`Digest`);
- a read that fails is `Network`, so an automatic run tries again soon, as it does when a server does not answer.

**What has run from a directory:** what your `linkupdate` suite shows, the signature refusal and a whole update. An expired or older channel, a changed boot file and a resumed file have run over HTTPS only, in the `update` suite. They take the same `stage()`.

So criterion 2 holds by construction, tested over HTTPS and read in the code for a directory. If you want those four to run from a directory too, the `linkupdate` suite can put each one into `data/link/rel/`, as the `update` suite does on its server.

## Point 2: the stale line

`docs/update/updater.md` and its Russian twin on `main` f763002 now say what `linkupdate` shows and which refusals have not run from a directory.
