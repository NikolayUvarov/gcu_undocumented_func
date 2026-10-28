# 800-KRN-0148 — With `dev`, `link` holds a client of the shell's commands, badged as the link's

**Type:** `init`, ABI (`SLOT_COMMANDS`, ABI 15) · **Owner:** kernel session · **Priority:** P1 · **Status:** in `main` (aa50204, landing round 8); the shell's side (APP) left · **Blocked by:** — for this part; the shell serving `SLOT_COMMANDS` with the link's rule is the tools track's · **Main task:** [800](800-mind-link.md) (the link track's) · **Constitution:** MC-2.3, MC-3.1, MC-3.8, MC-10.2

From the link track's request (`internal-dev-messages`, `KRN/2026-10-22T0356Z-LNK-a-shell-client-for-link-in-developer-mode.md`), for 800-LNK-0015.

## Problem

- **The maintainer, 2026-10-22:** "you must be able to run commands there".
- **What the link cannot do.** The tools track asked for the MacBook Pro's `ip`, `netstat -i`, `ps` and `dmesg`, and the link could not give them.
  - These are the shell's own commands, served by `shell.wit` `run(line)`.
  - `link` holds no client of the shell. In developer mode (800-KRN-0139) it starts programs by name, but runs no shell command.
- **Why `init` cannot simply lend one.** The shell makes its command endpoint itself, the first time a program asks for it. `init` knows no such endpoint, and `link` starts before the shell.

## Plan

1. **ABI 15: `SLOT_COMMANDS` (37).**
   - It holds the shell's command endpoint, which `init` makes as it makes a service's: a keeper, kept across the shell's restarts.
   - The shell gets the server side, with every right, as a service gets `SLOT_SERVICE`. The shell's slot 1 is its client of `init`, so it needs another one.
   - `SLOT_DYNAMIC` moves to 38. It is no launch slot.
2. **With `dev` in `link.conf`** (and only then), `init` gives `link` a client of that endpoint in `SLOT_SHELL` (30), badged `BADGE_SHELL_LINK` (1, `libmind::process`).
   - The shell tells the link's calls from a program's by the badge (`Received.badge`).
   - `init`'s developer-mode line names the shell's client with the others.
3. **Without `dev`, `link` is unchanged.**
4. **The shell's side is the tools track's** (LNK's request to APP, the same hour; the maintainer chose (a) for the commands that change the machine):
   - serve `SLOT_COMMANDS` when it holds it, in place of the endpoint it makes now;
   - apply the link's rule to `BADGE_SHELL_LINK`.

## Acceptance criteria

- [ ] `tests/runtime.rs` checks `SLOT_COMMANDS` (37), `SLOT_DYNAMIC` (38) and ABI 15.
- [ ] With `dev`, `stat caps` of `link` shows slot 30 on the shell's endpoint (the `link` suite's set of developer grants); without `dev`, nothing more (`linkupdate`).
- [ ] The shell holds slot 37, an endpoint it serves.
- [x] The gate passes (`land-aa50204e`, landing round 8).
- [ ] Once the tools track's side is in: `mindlink sh ip` returns the shell's `ip` (the link track's suite, 800-LNK-0015).

## Related

[800-KRN-0139](800-KRN-0139-the-links-developer-mode.md) (the developer mode), [800-KRN-0144](800-KRN-0144-a-service-replaced-from-data-link.md), [docs/profile/bootstrap.md](../docs/profile/bootstrap.md), [docs/api](../docs/api/README.md).
