# 750-EFF-0018 — The agent's name changed on the machine, over the cable

**Type:** session, tool · **Owner:** `EFF` · **Priority:** P2 · **Status:** open (2026-10-28) · **Blocked by:** `APP`: a shell command that sets the agent's hostname, taken from the link's client in developer mode (asked on `internal-dev-messages`, 2026-10-28) · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-2.3, MC-10.2

## Problem

The maintainer (2026-10-28): the agent's name must be changed on the machine itself, and without the machine's keyboard.

- **Where the name lives.** The agent reports the `hostname` line of the owner's `data/effector/effector.conf`. Since 750-EFF-0017, while that line names the machine an identity was copied from, the agent reports one of its own instead (`mind-n100` on the DEXP).
- **Nothing remote reaches it:**
  - the server cannot change the configuration (contract 1.0), and its alias is a server-side name the agent does not know;
  - `link` writes only `data/link/` and, with `dev`, `data/firmware/`;
  - the shell refuses its file commands to the link's client (`shell/src/clients.rs`).
- **So today** the owner edits the file at the machine's keyboard, then the agent is restarted.

## Plan

1. **`APP`: `effector name <hostname>` in the shell.**
   - It rewrites the `hostname` line of `data/effector/effector.conf`, or adds one, and leaves every other line as it is.
   - The name is checked as the agent checks it: 1 to 64 of `[A-Za-z0-9-.]`.
   - `effector name` alone says the name in the file.
   - It is logged.
   - **Over the cable:** it is taken at once from the link's client in developer mode, as the shell's other changes are (the maintainer's choice of 2026-10-22). At the console it is asked first, as the shell's changes are.
   - **Why not the shell's general file commands for the link:** those would also reach the owner's `data/mode.conf`, whose change stays the owner's act at the console (750-KRN-0143).
2. **The restart.**
   - The agent reads its configuration at each start. Over the cable, `effector485 submit replace unit=effector back=true`, or `mindlink replace effector --back`, starts it again from its boot image.
   - Whether the shell command should restart the agent itself is `APP`'s and `KRN`'s to say (a lifecycle client for one named service).
3. **EFF:** `docs/effector` says how to name an agent, at the console and over the cable. A copied identity is set aside as before (750-EFF-0017): a name the owner sets is never the foreign one, so it is used as written.

## Acceptance criteria

- [ ] `APP`'s command, with its host tests.
- [ ] The `effector` QEMU suite: a name set through the link's shell client is reported in the heartbeat after a restart.
- [ ] On the DEXP: `effector485 submit shell line="effector name <name>"`, then the restart, and the server lists the agent under that hostname. Evidence for the DEXP only (MC-12.1).

## Related

[750-EFF-0017](750-EFF-0017-an-identity-of-the-machines-own.md), `800-LNK-0015` (the shell over the link).
