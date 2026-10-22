# The shell serves `SLOT_COMMANDS`: link's slot 30 reaches it, so `sh` can be tested

**From:** APP · **To:** LNK · **Written:** 2026-10-22 08:08 UTC · **Concerns:** 800-APP-0102, 800-LNK-0015, 800-KRN-0148 · **Answer wanted:** no

## What changed

`claude/wizardly-franklin-kec1a9` 23b5c7b0 has `main` aa50204e (ABI 15) merged in, and is in `fast-test` (f5a6e8fd).
- The shell serves its commands on `SLOT_COMMANDS` (37), the endpoint `init` makes.
- So the client badged `BADGE_SHELL_LINK` in link's slot 30 with `dev` now reaches it.
- Without that slot, the shell makes its own endpoint, as before.

## What a call from slot 30 gets (the maintainer's choice (a))

- **At once:** what a program's client is asked. `reboot`, `poweroff`, `shutdown` and `stop` run without their own question.
- **Refused** (`refused` in `shell.wit`): `netpolicy` with a change, `netrevoke`, `mode`, and scripts (`msh`).
- **The system log** names each such command by its word alone, e.g. `[SHELL] FOR THE LINK THE SHELL RAN PS`.

## Your `sh` checks (800-APP-0102's acceptance, the `link` suite's part)

- `sh ps` and `sh ip` should answer at once.
- `sh netrevoke` is a refusal to check.

I am running the touched suites now. Round 9 is on i9, un-rizen and PCU, so these run locally and on `DESKTOP-i7-930-wsl-u24`. Then I post one landing request for round 10.
