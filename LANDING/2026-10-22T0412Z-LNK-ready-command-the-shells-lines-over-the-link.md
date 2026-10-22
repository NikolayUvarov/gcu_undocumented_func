# Ready: COMMAND, a line for the other end's shell (800-LNK-0015, the host's side)

**From:** LNK · **To:** LANDING · **Written:** 2026-10-22 04:12 UTC · **Concerns:** 800-LNK-0015

**Ready:** `claude/LNK-link` 97d48a321b716e0d8e4cdb034abf9acfc88f86d5

## What it brings

- **`linkproto`:** COMMAND (54) on the programs channel, a line of at most 240 bytes.
  - The answer is a run whose output and end OUTPUT reads, exit 1 with the shell's reason when it refused the line.
  - `Files::command` answers UNSUPPORTED by default, so MIND Core's `link` is unchanged until KRN's shell client and APP's rule exist. Both are asked on this branch, 2026-10-22.
- **`mindlink sh <line>`,** whose exit code is the line's; `serve --run` answers with `sh -c`.
- **`effector485`'s kind `shell`.**
- **Docs:** `docs/link/` protocol, mindlink and effector485, English and Russian.
- **800-LNK-0015** itself, with the maintainer's choice of 2026-10-22 recorded.

## `main` in it

f763002 (ABI 14).

## What was run

On the link track's WSL machine, x86, 4 CPUs:
- `scripts/mind_idl.py --check`: up to date;
- `linkproto`'s host tests: 18 passed, `shell_commands_over_a_lossy_line` among them;
- `mindlink`'s and `effector485`'s host tests: passed, `sh` and `shell` among them;
- `./02_build.sh` and the QEMU `link` suite: PASS, no part skipped.
