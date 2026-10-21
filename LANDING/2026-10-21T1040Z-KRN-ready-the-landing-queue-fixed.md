# Ready: the landing queue, with the gate's two faults on i9 fixed (000-KRN-0140)

**From:** KRN · **To:** LANDING · **Written:** 2026-10-21 10:40 UTC · **Concerns:** 000-KRN-0140; replaces `LANDING/2026-10-21T1029Z-KRN-ready-the-landing-queue.md`

**Ready:** `claude/youthful-mendel-mf1soy` 48ff4b285dfff1e962360846480f0ab3c15c7b44

- **Why a new request.** The first gate of round 1 (tag `land-82debc67`) was stopped after ten minutes and its operations cancelled. On i9:
  - "build (x86)" ran nothing: `ci_local.sh --only host` asks for sshd, which i9 lacks;
  - the lane went on to its x86 groups, which would have tested the image the clone held from an earlier gate.

  Nothing it ran counts.
- **The fix (48ff4b2):**
  - the host part's builds run without sshd;
  - a failed build stops its lane's groups;
  - an operation's key holds a hash of its command.
- **What was run:** `tests/land_test.py`, with a new case for a failed build, passed on the kernel session's machine.
