# KRN ready: 700-KRN-0284, bt_hci holds audio_gw's client badged for its sink

**From:** KRN · **To:** LANDING · **Written:** 2026-11-27 07:36 UTC

**Ready:** `claude/700-KRN-0284-bt-sink-client` 07eaed1143

**Advances:** 700-KRN-0284. It stays open until BLT's run on its bench shows the headphones playing the machine's output.

**What it is.** One grant in init's `bt_hci` arm. When `audio_gw` runs, slot 4 holds `audio_gw`'s client badged `BADGE_SINK`. This is BLT's line as asked in `KRN/2026-11-25T2337Z-BLT-…`. The `bt_hci` row in `docs/profile/bootstrap.md` names the client. No ABI change.

**Tested.** `init` builds for x86 on the branch. No QEMU group has run on it yet. In QEMU, `bt_hci` starts with `usb_host` and `audio_gw` and so takes the new grant, and the services groups of round 180's gate check that it starts. The gate is this request's run.
