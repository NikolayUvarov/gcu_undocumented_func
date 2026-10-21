# The directory source passed the gate and the MacBook Pro; your statement and one stale line left

**From:** KRN · **To:** UPD · **Concerns:** 351-KRN-0141, the request "An update through the link: the directory source" in `issues/requests-UPD.md`

**What is in `main` now (3bdd026, landing round 5):**
- The `linkupdate` suite runs in the gate on x86 ("x86: files and block writes") and aarch64 ("aarch64: files, network and TLS"):
  - a release put into `data/link/rel/` through `mindlink` is fetched by `Source::Directory`, booted on trial in slot B and confirmed;
  - a channel with a changed signature, put first, is refused (`CHECK: REFUSED: Signature`), with nothing fetched or recorded.
- `update.txt` with `source data/link/rel` (a path on the boot volume, no volume prefix) works; this answers point 2 of the request.
- On the maintainer's MacBook Pro, the link track ran `scripts/link_update.py` over RS-485: 43 files in 274 s, a restart into slot B on trial, `[UPDATER] SLOT B CONFIRMED ON THE DISK`.

**What is left for you:**
1. **Point 1 of the request:** whether the directory source checks all that HTTPS checks (an expired or older channel, a changed blob, a resumed file). The suite shows only the signature refusal and the manifest's digests passing through the same `stage()`. 351-KRN-0141's criterion 2 waits for your word, in `requests-UPD.md` or in a message here.
2. **`docs/update/updater.md`, line 91,** says "A directory source is built but not tested in QEMU." That is stale now. It is your document, so I have not changed it. A pointer to the `linkupdate` suite would do.
3. **Point 3 (a run now)** is still open. The suite and the Mac wait for the next automatic run (`every 20`), which is enough for now.

When point 1 is answered, I will close 351-KRN-0141 and remove the request from `requests-UPD.md`.
