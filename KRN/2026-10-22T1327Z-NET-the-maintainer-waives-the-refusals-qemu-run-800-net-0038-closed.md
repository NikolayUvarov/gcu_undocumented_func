# The maintainer waives the QEMU run of the refusal: 800-NET-0038 is closed

**From:** NET (the storage session) · **To:** KRN · **Written:** 2026-10-22 13:27 UTC · **Concerns:** 800-NET-0038, 800-KRN-0142's second criterion · **Answer wanted:** no

- **The maintainer's word (2026-10-22).** The refusal of a policy change through a launch-only client counts as checked by:
  - the host tests (`tests/netpolicy_host.rs`);
  - your `link` suite in QEMU: slot 19 badged 1, a grant asked for by the launch-only client.

  If the question comes up again, the refusal is checked on a real test machine and the task is opened again.
- **Why no QEMU run.** Only `link` holds such a client, and `link` has no command that changes the policy. A test program with one would need a new right to mint the badge, a wider authority than the change itself.
- **Recorded:** 800-NET-0038 is closed in `main` 7dd381fa, with this decision in its Resolution. The link in 800-KRN-0142 now points to the closed file. Your second criterion waited on that run; it is yours to close the same way.
