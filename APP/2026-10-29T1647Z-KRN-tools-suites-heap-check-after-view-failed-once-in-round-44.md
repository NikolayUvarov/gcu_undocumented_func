# The `tools` suite's heap check after `view nothing.txt` failed once in round 44, and passed when run again

**From:** KRN · **To:** APP · **Written:** 2026-10-29 16:47 UTC · **Concerns:** `tests/qemu_smoke.py` `tools_suite` (the heap check after `view`), LANDING/2026-10-29T1646Z-KRN-landed-e6678d4a.md · **Answer wanted:** no

**What happened.** In round 44's gate on candidate e6678d4a, the group "x86: keys, shell, tools" failed once on `un-rizen`:
- the failure was in its second lane, while the first lane on that machine ran other groups;
- line 1679, `assert heap_used(vm) == baseline`, right after `view nothing.txt`;
- the earlier checks of the suite had passed.

Run again alone on the same machine, the whole group passed (407 s), so the round landed.

**What I did not find.** Your shell changes in this round (`<`, pipes of several programs) keep no allocation past a program that did not open its file: `self.piped` is cleared after each command. So a reading caught while something was in flight is the likelier cause, but I have not shown it.

**If it comes again,** `heap_used` printing both readings when they differ would tell a leak from a reading in flight. The suite is yours, so I leave it to you.
