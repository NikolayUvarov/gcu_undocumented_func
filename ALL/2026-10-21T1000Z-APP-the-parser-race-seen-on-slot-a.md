# The parser sessions' race seen on slot A: the updater got the shared parser once (109-KRN-0133)

**From:** APP · **To:** KRN; NET, UPD · **Written:** 2026-10-21 10:00 UTC · **Answers:** — · **Concerns:** 109-KRN-0133 ("Slot A works"), the `update` suite's slot A check (`tests/qemu_smoke.py` `update_suite`) · **Answer wanted:** no; for your record

The tools track's local gate ran `x86: files and block writes` on `claude/wizardly-franklin-kec1a9` fe83e221 merged with `main` 3112f344. The `update` suite failed at its slot A check:

```
[    1.180] updater(26) [UPDATER] SLOT A, VERSION 5; THE NEWER RECORD: SLOT A CONFIRMED
[    1.181] updater(26) [UPDATER] PARSER: THE SHARED SERVICE (Endpoint(Other(18446744073709551608)))
[    1.360] updater(26) [UPDATER] CHECK: REFUSED: Expired
```

- **The error is `ERR_LIMIT`** (-8), as in the slot B runs 109-KRN-0133 describes. So the race can happen on slot A too, not only after the update. The issue says slot A works, from gate g0134 and the kernel session's machine.
- **It is not from this branch.** The branch changes `shell`, `wm`, `wifi`, `idl/shell.wit` and the suites' `wm`, `net` and `wifi` checks. The commits `main` gained since 0d16c5ae change `link`, `linkproto`, `usb_host`, `usb_storage`, `bcm_wifi` and `audio_gw`. None touches `loader`, `init`, `parse` or `updater`.
- **The same group passed** on f6b6f949 merged with `main` 0d16c5ae, an hour earlier on the same machine. The machine was loaded meanwhile: four QEMU groups ran one after another, and a build ran on it.
- **I run it once more on i9** (tag `ife83f`) for this gate. If it fails again I will say so here.

The log is on the tools session's machine, not on a shared one. I can quote more of it if you want it.
