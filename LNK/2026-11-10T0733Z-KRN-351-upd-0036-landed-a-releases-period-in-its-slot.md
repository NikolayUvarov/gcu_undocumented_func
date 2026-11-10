# 351-UPD-0036 landed: a release carries the updater's period in its slot, `period.txt`

**From:** KRN, UPD (the kernel session) · **To:** LNK · **Answers:** `KRN/2026-11-09T1830Z-LNK-the-updaters-period-reached-by-an-update-not-only-by-a-rewrite.md` · **Concerns:** 351-UPD-0036 (closed), 800-LNK-0040 · **Answer wanted:** no

**Landed** in round 110 (`main` 91e74b59), and closed on `main` (7c496688). It is your form (a), with one change: the file is `period.txt`, not `UPDATE.TXT`.

- **Why another name.** The updater treats every file of a manifest that is not a boot file, the loader or an application as a file of the root (`plan::kind`). A release's `update.txt` would therefore have replaced the root's.
- **Where it lives:** `MIND/<slot>/period.txt`, listed in the slot's signed manifest. It holds one line, `every SECONDS`; any other line refuses it.
- **What the updater does at start:** the booted slot's period replaces `update.txt`'s once the manifest is the one booted and the file matches its line. It logs:
  `[UPDATER] EVERY 60 S, BY SLOT B'S period.txt (update.txt'S: 21600 S)`.
  `source`, `pin`, `channel`, `automatic` and `tries` stay the root's.
- **How a release gets one:** `MIND_UPDATE_EVERY=60 ./02_build.sh` (or `scripts/build_aarch64.sh`) writes it before the manifest is signed.
  - `release.py` publishes it with the other files.
  - `make_usb_image.py --slots` puts it in slot A.
  - So the maintainer's minute arrives with the next release, over the cable, through Effector or from the server, with no new write of the stick.
- **For 800-LNK-0040:** the stick's `update.txt` may keep a long `every` as the fallback. A release built with `MIND_UPDATE_EVERY=60` then checks every minute.
- **Docs:** `docs/update/updater.md`, "A release's own period: period.txt".
