# NET ready: three checks round 129 showed on i9

**Ready:** `claude/111-NET-0076-nts-refused-race` 4ebd637236

**Advances** 111-NET-0076 and 000-NET-0077 (tests only); both close once this lands.

**What it changes** (`tests/qemu_smoke.py` alone, and the issues):
- **111-NET-0076:**
  - **The refused NTS servers' check** waits, up to 180 s, until each of the three servers' key establishment is logged. A server whose rules joined timed's grant after its first round, as 111-NET-0074 allows, comes in a later round.
  - **The sleep across the step** is counted on the host from just before the command is sent, the bounds moved with it to 25 s to 33 s, so the count no longer leans on the host's own one-second sleep.
- **000-NET-0077:** `browse_check` waits for the prompt after the question's take-down in the log with its lines joined (`joined()`): the screen's width had cut the take-down's line inside the text it waited after.

**Tested** on the maintainer's machines (QEMU under TCG):
- **At dd79ebc0:**
  - "x86: network, TLS and Effector" on x32-128 (1421 s, its other lane idle);
  - "aarch64: network, TLS and Effector" on PCU (1411 s).
- **At c57928a1:** "aarch64: network, TLS and Effector" on PCU (1420 s).
- **On i9:**
  - the `net` suite passed, `timed_check` and `browse_check` with it;
  - the `tls` suite then logged nothing for 40 minutes after its TLS client check, until the operation's limit. That check and the ones after it are not this branch's; the stop is recorded in 111-NET-0076.

`main` d8c0ec82 is merged in.
