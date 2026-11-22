# DRV landing request: the cameras' large frames, the dry rings said, an audio test program, and the IJG record that the JPEG code in `main` lacked

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-22 10:51 UTC · **Concerns:** 158-DRV-0082, 158-DRV-0083, 000-DRV-0093, 000-DRV-0094 (all advanced, all stay open); 158-ASR-0016 (closed); 158-DRV-0084 (its record of third-party code)

**Ready:** `claude/158-DRV-0082-two-cameras-and-their-modes` b437cb189

b437cb189 is e00b851f0 with `main` 1e9d8beb5 merged in (one conflict, `issues/README.md`, both sides' rows kept). Thirteen commits since 502d9a4f2, which landed in round 169.

## First, what `main` lacks and this mends

**Two files in `main` derive from the Independent JPEG Group's libjpeg release 6b and were not recorded as third-party code:**
- `libmind/src/jpeg.rs`: its forward DCT (`jfdctint.c`) and quality scaling (`jcparam.c`), in `main` since issue 093 (2026-10-06);
- `libmind/src/jpegdec.rs`: its inverse DCT (`jidctint.c`), in `main` since 502d9a4f2 landed in round 169. **That request of ours did not say so.**

Both are rewrites in Rust from memory of the C: no line was pasted, but they keep its constants, passes and shortcuts, so they are derived from it. This request records it:
- `THIRD_PARTY.md`: the source and the tarball's SHA-256, which file derives from which, what was changed, and the sentence the IJG asks for;
- `LICENSES/IJG.txt`, and `LICENSES/IJG-README-6b.txt`: the 6b README whole and unaltered (byte for byte the tarball's), on the careful reading of the IJG's condition (1);
- both files' headers name it; the built image carries all three (they are in its manifest);
- **a question for the maintainer** in `issues-human/README.md`, section 17: whether this meets the IJG's terms as he reads them. It is a legal question and his.

## What else it carries

- **158-DRV-0082:** `usb_host` spills a bulk stream's finished transfers, so a camera's uncompressed frames of 640x480 and larger no longer break at 128 KiB beside a second camera; `video_gw` drops the cameras of a `usb_host` that ended, at its first look; why a frame breaks is counted cause by cause; a bulk ring that stood empty is said with the wait `usb_host` was in.
- **158-DRV-0083:** the Ryzen's cameras' modes compared with Linux's lists.
- **000-DRV-0093:** `tests/audio_stub`, a test program that holds capture and playback open together, checked in QEMU's `usbaudio` suite.
- **000-DRV-0094, step 1:** every dry ring of an isochronous stream is said with its time, its length and the wait it fell in. No change of behaviour.
- **158-ASR-0016, closed:** the camera interfaces and the JPEG decoder in `docs/assurance/README.md` 1.3; the IDL fuzz run at the fixed seed (35 receivers, 211 types) found nothing. Done by DRV under the ASR owner's word.
- No system-call ABI change; no IDL change.

## Tested

- **On LEG9, the tree e00b851f0:** the image's build; the full `scripts/host_tests.sh`, whole on its first run (262 groups); QEMU `usb`, `ehci`, `iso`, `cameras`, `link`, `usbaudio` (with the audio program's check), `bench`, `wm`, `vfs`: 40 checks passed, none failed.
- **On the merged head b437cb189:** the image's build and the camera and USB host suites (camera, uvc, video_gw, video, usb_spill, usb_hub, usb_storage, image, jpeg, idl, idl_fuzz). The full host tests and the QEMU suites were not run again on it.
- **On the Ryzen today** (that machine, its two cameras and the speakerphone behind its hub; MC-12.1):
  - 3eb13b74c's `usb_host` by the cable's `replace`: fifteen minutes of capture and playback together with nobody on the cable: no dry ring while the streams ran, none missed, no error; the device's own agent counted no change in its queues;
  - five minutes with one read of the log over the cable in the middle: all 64 dry rings fell in that read, each waiting on the cable converter's bulk writes of 30 to 45 ms. So the lines name what starves the rings; the mending is 0094's later steps, not in this request;
  - the cameras with the spill: 640x480 uncompressed beside a second camera, 197 good frames and 7 broken (0 good of 84 before the spill); the remaining broken frames carry the camera's own error bit;
  - e00b851f0's services have been the boot image's own since 10:49 UTC (release 202611221044): both cameras found at boot. Nobody has looked at pictures on that build yet.
- **Not tested:**
  - the aarch64 groups on this head: they passed on 5046dfbc6 and on f3898fde6 (the branch before these commits); `usb_host` changed again since, so they matter and the gate runs them;
  - the camera's stream error code: these cameras do not answer the request (it stalls), so that path ran only to its refusal;
  - the 1-CPU suites and the QEMU groups not named.
