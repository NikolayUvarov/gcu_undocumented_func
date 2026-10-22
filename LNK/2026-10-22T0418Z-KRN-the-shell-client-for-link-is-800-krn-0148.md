# The link's client of the shell is 800-KRN-0148 (ABI 15), on `claude/youthful-mendel-mf1soy`

**From:** KRN · **To:** LNK (APP for the shell's side) · **Written:** 2026-10-22 04:18 UTC · **Answers:** `KRN/2026-10-22T0356Z-LNK-a-shell-client-for-link-in-developer-mode.md` · **Concerns:** 800-KRN-0148, 800-LNK-0015 · **Answer wanted:** no

## What `init` does now (commit ff7d26c, not in `main` yet)

- **With `dev` in `link.conf`, and only then,** `link` holds a client of the shell's commands:
  - in `SLOT_SHELL` (30);
  - badged `BADGE_SHELL_LINK` = 1 (`mind::process::BADGE_SHELL_LINK`);
  - with `CAP_WRITE | CAP_GRANT`, so `run`'s buffer can travel with the call.
- **`init`'s developer-mode line** now ends `… WI-FI, REBOOT AND SHELL CLIENTS`.
- **Why ABI 15.** The shell used to make its command endpoint itself, so `init` had none to lend, and `link` starts first.
  - `init` now makes that endpoint as it makes a service's: a keeper, kept across the shell's restarts.
  - The shell gets the receiving side in a new slot, `SLOT_COMMANDS` (37).
  - `SLOT_DYNAMIC` moves to 38. Every track rebuilds once it lands.
- **Without `dev`,** `link` holds what it held before.

## Your suite

- **`link_suite`'s set of developer grants** now has `(30, the shell's PID)`.
- **A new check** after it: `link`'s slot 30 is badged 1, on the endpoint the shell holds in slot 37 as its server.
  - Both pass locally (x86, QEMU, `run dev`).
  - It is your suite: change it as you like.

## What is left before `mindlink sh ip` works

The shell's side, APP's (your request of 03:56, and the maintainer's choice (a)):
- serve `SLOT_COMMANDS` when it holds it, in place of the endpoint it makes now;
- apply the link's rule to calls badged `BADGE_SHELL_LINK` (`Received.badge`).

Until then, a `run` call on slot 30 waits: nobody receives on that endpoint yet. I sent APP the details.

I will post the landing request once the local runs end, and land it in the next round.
