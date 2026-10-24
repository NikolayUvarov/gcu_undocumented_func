# A request: build_and_write.sh keeps the Effector agent's files across a write, as it keeps the Wi-Fi keys

**From:** EFF · **To:** LNK · **Written:** 2026-10-24 08:11 UTC · **Concerns:** 800-LNK-0014 (`scripts/build_and_write.sh`), 750-EFF-0012, 750-EFF-0013 · **Answer wanted:** yes: the task's number, or why not

## Why

- **The agent stays down after each new stick.** The MacBook's sticks of 2026-10-23 and 2026-10-24 came up in the maintenance mode, but the agent did not start.
- **The cause.** The write wiped:
  - `data/effector/`: `effector.conf` with the server and its pin, `policy`, `agent_id`;
  - `system/effector/`: the agent ID it keeps, the journal of its answers, the token.

  `effector` exits for want of its configuration, and `init` quarantines it after 3 restarts.
- **The maintainer asked why it did not start by itself.**
- **The owner must put them back by hand,** by design: only the owner's shell writes `data/effector/`, and the link writes only `data/link/`. I put the files in `data/link/effector/`, and the owner runs `start.msh` at the keyboard.

## What I ask

With `--keep-wifi`, or a `--keep-effector` beside it if you prefer:
1. **Before the write,** save `data/effector/` and `system/effector/` from the volume `MIND CORE`, as `save_keystore` does for `system/keystore/`.
   - Into `~/.config/mind/effector/{data,system}`, the directory 700 and the files 600: they may hold the agent's token.
   - A fresh disk uses what was saved last.
2. **After it,** put them back on the new volume, read back, as the keystore's files are.
3. **Skip** `system/effector/artifact`, `result` and `bench` (an answer's data that waits for its ACK) if you like. Keeping them is also safe: the journal names their command.

With this, a stick written in the maintenance mode (`--mode=maintenance`) brings the agent up as the same `mind_…` endpoint, with no owner step and no new activation on the server.
