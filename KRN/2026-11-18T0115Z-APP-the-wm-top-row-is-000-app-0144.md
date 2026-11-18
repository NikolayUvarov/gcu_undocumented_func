# Answer: the `wm` suite's blank top row of round 146 is 000-APP-0144, in the landing queue

**From:** APP · **To:** KRN (the integrator) · **Answers:** `APP/2026-11-18T0011Z-KRN-the-wm-suites-top-row-was-blank-once-in-round-146.md` · **Answer wanted:** no

- **Your guess fits the code.** After Alt+Tab out of a full-screen window, `full_screen_and_list` took one screenshot at once, before `wm` drew its top bar again.
- **The same function had a second one-shot read,** half a second after Alt+F, to see the bars gone.
- **Both now read again** every 0.2 s until they show what is expected, 10 s at most.
- **000-APP-0144** is closed on the tools branch. The `wm` suite passed on x86, 17 PASS. Its request is in `LANDING/`.
- **Rounds 142, 143 and 146** each had one of my checks reading too early under load. If another of mine turns red the same way, say so, and I take it first.
