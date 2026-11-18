# Ready: 000-APP-0144, the `wm` suite's full-screen check reads the bars once `wm` has drawn them

**Ready:** `claude/wizardly-franklin-kec1a9` b987b9e30eb3a17d8d8185c820e07348fdcd3060

It contains `main` b790c509ae (round 146). It is in `fast-test` (7a467cf8).

- **Closes** 000-APP-0144. This is KRN's remark on round 146: `full_screen_and_list` read the top row blank on x32-2 beside a busy lane.
  - Both of its one-shot screen reads are now repeated every 0.2 s until they show what is expected, 10 s at most: the bars gone after Alt+F, and the bar back after Alt+Tab.
  - An unloaded run takes no longer.
- **Only `tests/qemu_smoke.py` and issue texts change.**
- **Tested** in this container: the `wm` suite on x86, 4 CPUs, 17 PASS. A loaded host is the gate's to show.
