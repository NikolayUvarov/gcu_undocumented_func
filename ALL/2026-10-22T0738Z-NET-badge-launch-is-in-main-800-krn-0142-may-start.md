# `BADGE_LAUNCH` and `netpolicy.wit` 1.2 are in `main`: 800-KRN-0142 may start

**From:** NET · **To:** KRN, LNK · **Written:** 2026-10-22 07:38 UTC · **Answers:** `NET/2026-10-21T1221Z-KRN-the-launch-only-client-numbered-800-krn-0142.md` · **Concerns:** 800-NET-0038, 800-KRN-0142 · **Answer wanted:** no

- **What landed.** Round 8 put the broker's part of 800-NET-0038 into `main` (my a1488f4d, merged as 50bc8bd6):
  - `libmind/src/network.rs`: `BADGE_LAUNCH` = 1 and `may_change_policy(badge)`, true only for badge 0;
  - `idl/netpolicy.wit` 1.2 `denied`;
  - `netpolicy` refuses `revoke`, `add` and `remove` from a client badged `BADGE_LAUNCH` and logs `[NETPOLICY] <WHAT> REFUSED FOR PID <n> : A LAUNCH-ONLY CLIENT`; `prepare` from it is logged `, ASKED BY A LAUNCH-ONLY CLIENT`;
  - host tests: `tests/netpolicy_host.rs`.
- **For KRN (800-KRN-0142).** `init` may now mint `link`'s policy client with `BADGE_LAUNCH`. The QEMU check of 800-NET-0038 waits for that:
  - a program started over the link gets its grant;
  - an edit of the policy from `link`'s client is refused with the line above.
- **For LNK.** Nothing to change in `link`. Its launch-only client refuses edits once `init` mints it.
- **Closed in `main` c9300679:** 109-NET-0037, 109-UPD-0022, 109-UPD-0017, 109-NET-0010.
