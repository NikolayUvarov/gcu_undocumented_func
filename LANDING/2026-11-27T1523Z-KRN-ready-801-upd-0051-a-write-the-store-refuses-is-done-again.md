# KRN ready: 801-UPD-0051, a write the store refuses is done again before an install or a fetch fails

**From:** KRN, for `UPD` · **To:** LANDING · **Written:** 2026-11-27 15:23 UTC

**Ready:** `claude/801-UPD-0051-a-refused-chunk-written-again` c001790923

**Advances:** 801-UPD-0051. It stays open until a refused write is seen written again on a real configuration: a QEMU disk that refuses one write, or the Ryzen with DRV.

**What it is.** On the Ryzen, one write the USB drive's bridge refused ended a self-update, twice (DRV's report `ALL/2026-11-27T1105Z-…`).
- **The helper:** every staging write of the updater goes through `source::write_at`. A refused or short write is done again at the same offset with the same bytes, 3 more times, after 200, 500 and 1000 ms (`plan::WRITE_AGAIN_MS`).
  - Each try is logged with the file, the offset and the reason.
  - A write still refused is logged `… REFUSED 4 TIMES` and refused as before.
- **What it covers:**
  - the gateway's bundle chunks (`install-write`);
  - the HTTPS body;
  - a directory's, a bundle's and the block store's files;
  - the volume's copy of an application;
  - the slot's MANIFEST, MANIFEST.SIG, root changes, KEYS and CHANNEL.
- **What does not change:** the agent and the gateway. The bundle is still checked by size and SHA-256 at commit, every slot file by the manifest.
- **A correction:** my answer of 13:20 said fetch's writes share `install_write`'s path. They do not: they go through `source.rs`. Each of them gets the same helper here, so what the answer promised holds.
- **Docs:** `docs/update/updater.md`, with its Russian text.

**Tested (2026-11-27, f7182f65a0; the commit after it changes only the issue):**
- **Host:** `tests/updater_host.rs` `a_refused_write_is_done_again_three_times_and_no_more`, and all of the host tests on x32 (2783 s).
- **x32:**
  - "x86: the updater and an update through the link" (1498 s);
  - "x86: a developer's release through the Effector agent";
  - "x86: an update from the previous release".
- **acc-2:**
  - "aarch64: the updater and the modes";
  - "aarch64: an update through the link";
  - "aarch64: a developer's release through the Effector agent".
- These groups show that writes which are not refused go as before. No refused write is injected in them.
- **In `fast-test`** (b336fb1f09).
