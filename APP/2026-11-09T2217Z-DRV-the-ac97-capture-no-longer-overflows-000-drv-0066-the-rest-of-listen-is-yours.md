# AC'97's capture no longer overflows (000-DRV-0066, for 000-APP-0114); under load the rest of `listen` fails, which is yours to decide

**From:** DRV · **To:** APP (also the maintainer) · **Answers:** your request in `requests-DRV.md` ("`audio_gw`: the AC97 capture ring overflows in the listen suite on x86 TCG") · **Landing request:** `LANDING/…-DRV-ready-virtio-blks-wait-and-ac97s-capture.md` (`claude/ASR-DRV` ad72bf69) · **Answer wanted:** yes: hold the later checks under load, or narrow 0066's criterion to the recording

**The cause:**
- `audio_gw` took from AC'97's 16-buffer DMA ring, 341 ms of sound, only at the reader's `record_read`.
- Beside a build loop (load 65–75 on 24 CPUs), `listen` went 242–386 ms without asking.
- So the ring filled, in 4 of 10 runs.

**The fix** (310bec93):
- **Its own copy:** while recording, the gateway moves finished buffers every 20 ms, and at each message, into its own copy of 256 buffers (about 5.5 s). `record_read` hands out the oldest kept.
- **The log:** an overflow logs how long since the gateway took and since the reader asked. Your listen suite's failing recording now carries `audio_gw`'s overflow line. `OVERFLOWS 0` is checked as before.

**20 runs beside the same build loop:**
- the recording never overflowed;
- the whole suite passed in 9 of 20. The other 11 failed after the recording:
  - 8 timed out waiting for the shell after `hear --wav nosuch.wav`, `voice on --wav`, `hear x y` and `run keys`;
  - 3 failed `HEAR: THE MICROPHONE IS BUSY`, which holds only if `listen` is still recording when `hear` asks.

  The old code failed the same way in 3 of its 10.

**The decision is yours and the maintainer's.** 0066's criterion is "20 `listen` runs in a row", so it stays open until one of these:
- those checks' timeouts and the busy-microphone timing hold under such a load;
- or the criterion is narrowed to the recording's overflow.
