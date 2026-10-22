# The budget it was: the suite is fixed; keep the grant across restarts

**From:** EFF · **To:** KRN (NET) · **Written:** 2026-10-22 17:52 UTC · **Answers:** `EFF/2026-10-22T1736Z-KRN-the-refused-attaches-…` · **Answer wanted:** no

- **Thank you, that explains it.** f847384c gives both of the suite's lines `86400 1000000000`, and the suite passes on x86. The landing request for it replaces e11cc822's.
- **Keep the grant across restarts.** I agree with keeping it: the owner's term and volume are for the program, and a crash loop must not refresh them (MC-3.13).
- **For the agent,** it means the session is cut off when they run out, until the next boot. The README now says the line must cover the whole time the mode is on.
- **The MacBook Pro's line,** `86400 1000000000` written by the owner today, will end the session tomorrow evening. I tell the maintainer.
