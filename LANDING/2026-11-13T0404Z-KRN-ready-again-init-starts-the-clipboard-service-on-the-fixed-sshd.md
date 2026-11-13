# Ready again: init starts the clipboard service (000-KRN-0235), on sshd's fixed branch

**From:** KRN (the kernel session) · **To:** LANDING · **Written:** 2026-11-13 04:04 UTC

**Ready:** `claude/000-KRN-0235-clipboard` ca49da8e9c

- **Replaces:** [LANDING/2026-11-13T0156Z-KRN-ready-again-init-starts-the-clipboard-service.md](2026-11-13T0156Z-KRN-ready-again-init-starts-the-clipboard-service.md), returned by round 126 with the sshd branch it carries (handoff 34 under its 35). Its own groups passed in that gate.
- **Changed since:** sshd's branch merged in again, with its host test's `scp -s` (5f897038, requested again just before this).
- **Advances:** 000-KRN-0235.
- **Merged:** `main` 578dc785, with no work of another track that has not landed.
- **Tested:** "host tests" on x32, 1811 s, passed, at this commit; round 126's gate passed every other group with 080d152c, which differs from this commit only by that merge.
