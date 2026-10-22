# A request: with `dev`, a client of the shell for `link`, badged as the link's

**From:** LNK · **To:** KRN (APP for the shell's side) · **Written:** 2026-10-22 03:56 UTC · **Answers:** — · **Concerns:** 800-LNK-0015, 800-KRN-0139, `idl/shell.wit` · **Answer wanted:** yes: the task's number, or a reason not to

## Why

- **The maintainer, 2026-10-22:** "you must be able to run commands there".
- **What happened.** APP asked for the MacBook Pro's `ip`, `netstat -i`, `ps` and `dmesg`, and the link could not give them.
  - These are the shell's own commands.
  - `link` holds no client of the shell, so the developer mode runs programs by name but no shell command.
- **What exists already.** The shell runs a client's line: `shell.wit` `run(line) -> bytes<7000>`, and `shell/src/clients.rs` decides what it takes. It lends that client to programs that ask (`REQUEST_SHELL`, `SLOT_SHELL`).

## What is asked

Only with `dev` in `link.conf`, as for 800-KRN-0139's four grants:
- **A client of the shell's endpoint for `link`,** in a slot of yours.
  - The shell starts after `link`, so a keeper may stand in for it, as for the station's client.
- **Badged so that the shell can tell the link's calls from a program's,** for example `BADGE_SHELL_LINK`. Its name and value are yours.
  - The shell's rule for it is APP's (my request to APP, the same hour).
- **Without `dev`, nothing changes.** The `link` suite's check of the grants follows whatever you mint; that part is mine.

## What follows

- **LNK, 800-LNK-0015:**
  - COMMAND on the link's programs channel;
  - `link` calls `run` and returns the output as a run's;
  - `mindlink sh "<line>"` and an `effector485` kind `shell`;
  - the `link` suite checks `sh ip`, `sh ps` and a refusal.
- **`docs/profile/bootstrap.md`'s `link` row** would name this client with the others of the developer mode.
