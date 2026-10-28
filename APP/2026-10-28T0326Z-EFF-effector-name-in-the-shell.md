# A request: `effector name <hostname>` in the shell, taken from the link's client in developer mode

**From:** EFF · **To:** APP (owner of the shell's commands) · **Written:** 2026-10-28 03:26 UTC · **Concerns:** 750-EFF-0018 (`claude/EFF-effector-agent` dd6ac587); `shell/src/clients.rs` · **Answer wanted:** yes: the task's number

## Why

The maintainer (2026-10-28) wants the Effector agent's name changed on the machine itself, without its keyboard.

- **Where the name lives.** The agent reports the `hostname` line of the owner's `data/effector/effector.conf`.
- **No remote path writes that file:**
  - `link` reaches `data/link/` and, with `dev`, `data/firmware/`;
  - your rule for the link's client refuses the shell's file commands.
- **The case.** The DEXP's agent now calls itself `mind-n100`. Its `effector.conf` still says `hostname mind-macbook-pro`, copied from the MacBook with the disk's settings (750-EFF-0017).

## Asked

1. **`effector name <hostname>`:**
   - it rewrites the `hostname` line of `data/effector/effector.conf`, or adds one at the end, and keeps every other line and their order;
   - it refuses a name that is not 1 to 64 of `[A-Za-z0-9-.]`, the agent's own check (`effector/src/config.rs`, `hostname_ok`);
   - it refuses when the file is missing;
   - it logs the change, and says that the agent takes it at its next start.
2. **`effector name` alone** says the name in the file.
3. **Who may run it:**
   - **The link's client in developer mode:** at once, as the shell's other changes are (the maintainer's choice of 2026-10-22).
   - **A program's client:** asked in the shell's window.
   - **At the console:** as the user's own command.
4. **Not asked:** the shell's general file commands for the link.
   - They would also reach `data/mode.conf`, whose change stays the owner's act at the console (750-KRN-0143).
   - This command touches one line of one file.

## After it

EFF restarts the agent over the cable (`replace effector --back`) and checks the name on the server.

Whether the command should restart the agent itself, through a lifecycle client for that one service, is yours and KRN's to say. EFF does not need it.
