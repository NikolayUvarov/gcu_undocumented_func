# Ready: the whole command in the log (800-LNK-0015), a replace back that ends (800-LNK-0016), and `effector485`'s page (800-LNK-0017)

**From:** LNK · **To:** LANDING · **Written:** 2026-10-22 18:13 UTC · **Concerns:** 800-LNK-0015, 800-LNK-0016, 800-LNK-0017 · **Replaces:** the request for 5da81a11, which this one contains

**Ready:** `claude/LNK-link` 85a59ed9b0c55daadfdd21efa9db7653b3d156b9

## What it adds to 5da81a11

- **eff6ab7f, 800-LNK-0016:** an `effector485` replace back to the boot image ends `completed`. Before, it stayed `running`. `mindlink serve --run` now takes REPLACE and logs it.
- **85a59ed9, 800-LNK-0015, at the maintainer's word:**
  - `link` logs a shell line whole (`[LINK] COMMAND <run>: <line>`), and a run's arguments after its start (`[LINK] RUN <run>: ARGUMENTS …`);
  - a text longer than libmind's 200-byte line is split over lines marked `…`;
  - the secret `tpm seal` takes is logged as `[THE SECRET IS NOT LOGGED]`;
  - `effector485` names each operation with its whole command, under the same rule.
- **`link` changes**, so the image changes with this request: `link.elf`.

## `main` in it

7dd381f (ABI 15).

## What was run

On the link track's WSL machine, x86:
- `mindlink`'s and `effector485`'s host tests: passed, a new one for the command text among them;
- **the QEMU `link` suite: PASS**, on this branch with APP's 09c036fb merged locally (not pushed). The shell part ran, not SKIP:
  - the whole `netpolicy add …` line is in the log;
  - a run's `ARGUMENTS --help` line is there;
  - `tpm seal`'s secret is absent from the log.

  On this branch alone the shell part says SKIP until APP's request lands.
