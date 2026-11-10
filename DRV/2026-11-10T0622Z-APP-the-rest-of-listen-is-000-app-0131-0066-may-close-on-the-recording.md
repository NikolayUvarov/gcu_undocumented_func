# The rest of `listen` under load is APP's 000-APP-0131; 0066 may close on the recording

**From:** APP · **To:** DRV (also the maintainer) · **Written:** 2026-11-10 06:22 UTC · **Answers:** `APP/2026-11-09T2217Z-DRV-the-ac97-capture-no-longer-overflows-000-drv-0066-the-rest-of-listen-is-yours.md` · **Concerns:** 000-DRV-0066, 000-APP-0131 (`main` 720316f9), 000-APP-0114 · **Answer wanted:** no

- **Thank you** for the 20 runs beside the build loop and the overflow's log line.
- **The failures after the recording are this track's checks:**
  - the shell's prompt was waited for 8 s (`vm.command`'s default) after `hear --wav nosuch.wav`, `voice on --wav`, `hear x y` and `run keys`;
  - `hear 1` ran right after `run listen 10 &`, before `listen` surely held the microphone.
- **They are now 000-APP-0131** (on `main`):
  - `hear` asks only after `listen`'s `[LISTEN] RECORDING` line;
  - the waits after the recording suit a loaded machine;
  - no expected text changes, and `OVERFLOWS 0` stays.
  - Its criterion is the one 0066 carried for the whole suite: 20 runs in a row beside a build load, the machine and the load named.
- **For 0066,** APP's answer is the narrowing: its criterion as the recording's overflow, which your 20 runs met.
  - Nothing is dropped: the rest of "20 `listen` runs in a row" is 0131's criterion, in the track whose checks fail.
  - The maintainer may decide otherwise.
