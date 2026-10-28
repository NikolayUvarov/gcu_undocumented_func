# Ready: several machines on one RS-485 line, and proprietary firmware over the cable

**From:** LNK · **To:** LANDING · **Written:** 2026-10-28 03:45 UTC · **Concerns:** 800-LNK-0026, 800-LNK-0023 (the write script), 800-LNK-0027 · **Replaces:** LNK's request for ee664728 (3179c15f, not landed; this branch holds it)

**Ready:** `claude/LNK-link` 1fabf12f50cada91a0adec19598f31dc17381a8b

## Land it with or after KRN's 2230Z request

This branch merges `claude/youthful-mendel-mf1soy` up to 85b39c18: 550-KRN-0172 (ABI 21) and 800-KRN-0173, which 800-LNK-0027 needs. KRN's own request is `LANDING/2026-10-27T2230Z-KRN-ready-again-550-krn-0172-and-800-krn-0173.md`.

## What it brings

- **800-LNK-0026:**
  - the protocol's version 2 with the slave's address;
  - `address=` in `link.conf`, `mindlink --address`;
  - `effector485` serving several machines on one cable.
- **The write script:** `build_and_write.sh` goes on at once with a disk that holds no MIND CORE volume.
- **800-LNK-0027:**
  - in developer mode `link` shows `data/firmware/` as `firmware/`, in init's slot 7 scope;
  - `scripts/proprietary.sh link PORT | effector485` puts the drivers' firmware;
  - `effector485 help` names `shell` and `replace`;
  - the docs are updated in both languages.

No ABI change of its own. No IDL change.

## `main` in it

e6c5e091.

## What was run on the branch

- **`./02_build.sh`:** passed (2026-10-27).
- **Host tests:** `link_names` (4); `tools/mindlink` cargo tests (800-LNK-0026).
- **QEMU x86:** `link` and `linkupdate` passed. The link suite now puts, reads, lists and removes a file in `firmware/test/`.
- **On the DEXP:**
  - a release with this `link` and KRN's `init` was put over the cable (slot B, confirmed);
  - `proprietary.sh link effector485` put the Bluetooth controller's files into `data/firmware/rtl_bt/`, read back with their pinned SHA-256.
