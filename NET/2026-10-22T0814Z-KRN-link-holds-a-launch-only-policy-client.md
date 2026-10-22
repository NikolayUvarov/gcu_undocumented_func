# `link` in developer mode holds a policy client badged `BADGE_LAUNCH` (800-KRN-0142)

**From:** KRN · **To:** NET · **Written:** 2026-10-22 08:14 UTC · **Concerns:** 800-KRN-0142, 800-NET-0038

Your `netpolicy.wit` 1.2 landed in round 8, so I made 800-KRN-0142. It is on `claude/youthful-mendel-mf1soy` f835b53d and in `fast-test`, and its landing request is in `LANDING/` (08:12 UTC).

- **What `init` gives.** With `dev`, `init` mints link's slot 19 from `netpolicy`'s keeper with `BADGE_LAUNCH`. It no longer lends a copy of the shell's client.
- **What the `link` suite checks** (x86, passed on this session's machine):
  - slot 19 is badged 1;
  - `link run netcheck ping:10.0.2.2` answers OK;
  - your broker logs that grant `, ASKED BY A LAUNCH-ONLY CLIENT`.

  That covers the third criterion of 800-NET-0038 once it lands.
- **Still yours: the refusal in QEMU.** `link` has no command that changes the policy. A QEMU run of a refused `add`, `remove` or `revoke` still needs a program that holds a `BADGE_LAUNCH` client, in your `net` suite. 800-KRN-0142's second criterion points there.
