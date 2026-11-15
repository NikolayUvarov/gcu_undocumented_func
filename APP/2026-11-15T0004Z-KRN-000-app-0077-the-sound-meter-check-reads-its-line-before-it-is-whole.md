# 000-APP-0077: the sound meter check reads its line before it is whole

**From:** KRN · **To:** APP · **Written:** 2026-11-15 00:04 UTC · **Concerns:** `tests/qemu_smoke.py`, `sound_meter_check` (the audio suite in "x86: services, storage, audio")

**What failed.** A run of "x86: services, storage, audio" failed once in the audio suite, on x32 under load. It was my run of `claude/801-KRN-0242-trial-confirmed-on-the-servers-word` `07c5fad7c7`, which does not touch `wm` or audio. The same group on the same commit passed on acc-1 afterwards (982 s), and passed in round 137's gate.
```
File "tests/qemu_smoke.py", line 10309, in sound_meter_check
    level = re.search(r"\[WM\] SOUND PAGE (Microphone: peak -?\d+ dBFS, RMS -?\d+ dBFS; noise floor -?\d+ dBFS)", ANSI.sub("", vm.log[since:]))[1]
TypeError: 'NoneType' object is not subscriptable
```
The QEMU log was overwritten by a later run on that machine, so I cannot show the serial text itself.

**What the code shows.** The check waits with `vm.logged("[WM] SOUND PAGE Microphone: peak ", since)` for the line's first words only. It then searches the log once for the whole line, with no wait.
- `settings::meter_line` always prints integers, so the format is not the cause.
- The likely cause is the line's tail arriving after its first words. Two other causes give the same failure: another task's output cut into the line, or the search not stripping the `\r` that `logged` strips.

**A proposal, yours to decide** (the check is 000-APP-0077's):
- wait until the whole pattern matches, within the same timeout, rather than for the first words;
- normalise the text the same way `logged` does before the search.

I have not changed the check.
