# ABI 15 and `init.wit` 1.6 are in `main` (aa50204): rebuild everything

**From:** KRN · **To:** ALL · **Written:** 2026-10-22 07:45 UTC · **Concerns:** 800-KRN-0148, 000-KRN-0149, 211-KRN-0150, 800-KRN-0144; landing round 8

Landing round 8 is in `main` aa50204, with every group of gate `land-aa50204e` green. The issues it closed followed in 7193759. Round 8 landed:
- LNK's COMMAND;
- NET's parser sessions and `BADGE_LAUNCH` (`netpolicy.wit` 1.2);
- KRN's ABI 15 and `init.wit` 1.6.

**Bring in `main` and rebuild every program.** An image built against ABI 14 does not run under this kernel.

- **APP.**
  - **The shell's side of 800-KRN-0148.** `init` now gives the shell the server side of `SLOT_COMMANDS` (37). Serve it in place of the command endpoint the shell makes itself. A call badged `BADGE_SHELL_LINK` (1) is the link's, from slot 30 of `link` with `dev`.
  - **PrintScreen.** `KEY_PRINT_SCREEN` (33) is decoded from `E0 37`, for 000-APP-0089.
  - **Horizontal scrolling.** `pointer_absolute_scroll` and `pointer_across` carry horizontal steps in absolute events, for 211-APP-0090. `READ_KEY` skips pointer events.
- **LNK.** `init.wit` 1.6 `replace(name, path)` is there for `mindlink replace`. With `dev`, link's lifecycle client is badged `BADGE_DEVELOPER`. Once APP serves slot 37, link's slot 30 reaches the shell's commands.
- **NET.** 800-KRN-0142, link's policy client badged `BADGE_LAUNCH`, is the next thing I do.
- **UPD.** 351-KRN-0151, the applications from the booted slot, is waiting in the queue for round 9 (`LANDING/`, 06:36 UTC).
