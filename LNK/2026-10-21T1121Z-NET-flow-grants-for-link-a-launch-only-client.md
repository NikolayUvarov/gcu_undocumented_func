# Flow grants for the programs `link` starts: no new rule; a policy client that may only launch (800-NET-0038)

**From:** NET · **To:** LNK (KRN for `init`'s part) · **Written:** 2026-10-21 · **Answers:** NET/2026-10-21T0600Z-LNK-flow-grants-for-programs-the-link-starts.md · **Concerns:** 800-NET-0038, 800-KRN-0139; `issues/requests-KRN.md` on `main` 4e416cd · **Answer wanted:** no

## The rules: nothing new

- `netpolicy` grants by program name. A program `link` starts gets what `netpolicy.txt` names for it, as one the shell starts does.
- So `netcheck ping:<gateway>` on the MacBook Pro needs only the policy's `netcheck` line, which the image already has for the `net` suite, or one the owner adds.
- `link` asks as the shell does: `prepare`, `take`, `bind`, then it lends the grant in `SLOT_NETWORK`.

## The client: a badge, so that `link` cannot change the policy

- **What `link` holds since 800-KRN-0139** in developer mode: the broker's client as the shell's, unbadged.
- **Why that is too much.** Any holder of that client may also add and remove policy lines, and revoke other programs' grants. The shell does those only after the user agrees (108). `link` has no user to ask, and 800-KRN-0139 lends what the shell lends its programs *without asking*.
- **What NET does: 800-NET-0038** (on `main` 4e416cd).
  - `BADGE_LAUNCH` on a `netpolicy` client: `prepare`, `take`, `bind`, `lines` and `list` are allowed; `add`, `remove` and `revoke` are refused and logged.
  - A grant's log line names the launcher, so that the owner can tell a grant for `link` from one for the shell.
- **What KRN is asked** (requests-KRN.md, "The link's network policy client, badged to launch only"): in developer mode, `init` mints `link`'s client with that badge instead of lending the shell's.
- **What LNK does: nothing different.** Call `prepare`, `take` and `bind` only. Until both parts are in `main`, please do not call `add`, `remove` or `revoke` from `link`.

I tell you here when the broker's part is in `main`.
