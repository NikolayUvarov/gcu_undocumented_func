# Ready: `link` runs the shell's lines (800-LNK-0015) and replaces a service (800-LNK-0016)

**From:** LNK · **To:** LANDING · **Written:** 2026-10-22 14:12 UTC · **Concerns:** 800-LNK-0015, 800-LNK-0016; 800-KRN-0144's first criterion, 800-KRN-0148

**Ready:** `claude/LNK-link` f477bf8f7f0f25d6ab446ddca2d095479d8f9e78

## What it brings

- **800-LNK-0015, `link`'s side:**
  - with `dev`, COMMAND is answered with a run;
  - then `shell::run` on KRN's slot 30, 60 s at most. Its answer is the run's output, or exit 1 with the shell's reason. Only the command's word is logged;
  - `mindlink sh` waits for a silent other end;
  - **the `link` suite** runs `sh ip`, `sh ps` and a refused `sh netpolicy add`, and says SKIP until the shell takes the link's lines (APP's 800-APP-0102, queued).
- **800-LNK-0016:**
  - REPLACE (55) on the programs channel; `link` asks `init.wit` 1.6 `replace` on its developer-mode lifecycle client;
  - `mindlink replace <unit> <file>`, `--back`;
  - **the `link` suite:** `rtc` replaced from a copy of its image, `init`'s `NOT VERIFIED` line with the digest, `date` still answered, back to the boot image, `logd` refused. That is 800-KRN-0144's first criterion.
- **Docs:** `docs/link/` protocol and mindlink, English and Russian.

## `main` in it

7dd381f (ABI 15).

## What was run

On the link track's WSL machine, x86, 4 CPUs:
- `linkproto`'s host tests: 19 passed, `replace_over_a_lossy_line` and `shell_commands_over_a_lossy_line` among them;
- `mindlink`'s and `effector485`'s host tests: passed;
- **the QEMU `link` suite: PASS.**
  - On this branch: `sh` said SKIP (no APP rule yet), and the replace case passed.
  - With APP's `claude/wizardly-franklin-kec1a9` merged locally: the `sh` checks passed as well.

**Order with APP's request:** either can land first. Once both are in, the suite's `sh` part runs instead of SKIP.
