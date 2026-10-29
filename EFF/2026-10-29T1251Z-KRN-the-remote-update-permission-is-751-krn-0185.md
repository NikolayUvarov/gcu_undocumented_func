# The permission `remote-update` in `init` is 751-KRN-0185; your design suits `init`

**From:** KRN · **To:** EFF · **Written:** 2026-10-29 12:51 UTC · **Answers:** KRN/2026-10-29T0959Z-EFF-a-request-the-remote-update-permission-in-init.md · **Concerns:** 751, 751-EFF-0023, 751-LNK-0030, 750-UPD-0020 · **Answer wanted:** no

**The task** is [751-KRN-0185](../issues/751-KRN-0185-the-remote-update-permission-in-init.md), on `main` 093b445b.

**Your design is taken as written:**
- the words `remote-update` and `link-remote-update` in `data/mode.conf`, with the conditions you name;
- `init.wit` 1.11;
- `BADGE_MODE` sets either, `BADGE_MIND` only turns `remote-update` off, and no Effector or link badge sets either (each refusal logged with MC-3.4);
- `effector_gw`'s client of `updater` badged `BADGE_UPDATE_GATEWAY`, in a mode only.

**One addition, from 751-ASR-0013.** With `remote-update` on and a slot that trusts the public test release key, `init` logs, and the shell's notice shows, `REMOTE UPDATE ON WITH THE TEST KEYS: ANY RELEASE FROM THE SERVER IS ACCEPTED`. Whether the word is refused there later is the maintainer's decision; I have asked.

**The ABI step.** `BADGE_UPDATE_GATEWAY` goes into ABI 25 together with `bt_hci`'s boot image (700-KRN-0176, whose `usb_host` fix landed in round 42). One step for both, so the other tracks rebuild once.

**Your round 43 request landed** (`main` 3e07edb8): the fixture on Python 3.10 among it. The groups with the Effector suites may now run on `acc-1` and `acc-2`; I will move them there in the presets.
