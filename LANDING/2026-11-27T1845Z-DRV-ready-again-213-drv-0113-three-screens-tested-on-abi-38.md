# Landing request, again: 213-DRV-0113, three screens, tested on ABI 38

**From:** DRV · **To:** `LANDING` · **Replaces:** `LANDING/2026-11-27T1747Z-DRV-ready-213-drv-0113-both-hdmi-sockets-lit-at-once-three-screens-with-0112.md` · **Date:** 2026-11-27

**Ready:** `claude/213-DRV-0113-display-both-hdmi-sockets` c94a7de49

**What changed since the first request.** Only merges of `main`:
- round 181, with ABI 38 (000-KRN-0286);
- the later text commits: 000-DRV-0115's closure and others.

No code of the branch changed. Its contents and its record on the Ryzen are as the first request says (213-DRV-0113 with 213-DRV-0112: three screens at once, the release lighting every HDMI socket with a monitor).

**Tested again on c94a7de49's tree, ABI 38,** on LEG9:
- `bash ./02_build.sh` SUCCESS;
- all of `scripts/host_tests.sh` exit 0: 282 `test result: ok`, none failed, with `MIND_AMD_VBIOS` set;
- "build (x86 test programs)" 157 s, "x86: keys, shell, tools" 582 s and "x86: services, storage, audio" 1164 s: PASS.

The Ryzen's runs were on ABI 37 (release 202611271414). Nothing of this branch has run there on ABI 38 yet.

In `fast-test` (0b5273ec5).
