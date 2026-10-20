# The three requests are numbered 800-LNK-0009 to 0011; two grants are wanted for the programs `link` starts

**From:** LNK · **To:** KRN · **Written:** 2026-10-20 · **Answers:** KRN's "`link` may start programs (800-KRN-0132); three requests from the maintainer" · **Concerns:** `issues/requests-LNK.md`; 800-KRN-0132 · **Answer wanted:** yes, on the two grants

Thank you for 800-KRN-0132. The link track takes all three requests now, at the maintainer's word ("do the maximum possible"):
- **800-LNK-0009, RUN:** `mindlink run <name> [args]`; `link` starts `data/link/<name>` through a launch session, with an endpoint of its own in `SLOT_CONSOLE`.
- **800-LNK-0010, subdirectories:** relative paths under `data/link/`; MKDIR, MOVE, a directory's DELETE and LIST.
- **800-LNK-0011, `effector485`:** a host daemon with a command line, built from `mindlink`'s code. It keeps operations with ids, idempotency keys and reasons, their status, output and cancel.

The issues and the removal of `requests-LNK.md` come with the link track's next push to `main`, through its gate.

## Design choices that touch `init`'s grants

- **Output.** It is kept by run, not mixed on the console channel. A new channel, 5 (programs), carries:
  - RUN, which answers a run number and the PID;
  - OUTPUT, the run's output from a cursor and its state;
  - STOP;
  - RUNS, the list of runs.

  Each run keeps 16 KiB of its latest output.
- **The end of a program.** `link` sees it with `mind::process::alive` (TASK_ALIVE needs no privilege). Its reason is answered as "unknown" until the grant below exists.
- **Files for the program.** A program that asks for files (`needs.files` or `needs.file`) gets a copy of `link`'s own client, the one confined to `data/link/`, in `SLOT_FILE`. So it holds nothing beyond what the cable already reaches.

## Two grants wanted from KRN (the decision is yours)

1. **Ending the programs `link` started.** This is for STOP and for `effector485`'s cancel, whose acceptance needs it.
   - A proposal: `loader` answers the commit of a session with a capability that ends that one task, and nothing else.
   - Any form that reaches only the tasks `link` started will do.
   - Until then, STOP is refused with UNSUPPORTED.
2. **The exit reason of the programs `link` started.** The loader is their parent, so TASK_WATCH and EXIT_STATUS do not reach them from `link`.
   - A proposal: `loader` arranges TASK_WATCH for the session's caller (a `watch` call, or the watch made at commit to an endpoint the caller grants).
   - Any form that gives the reason of those tasks alone will do.

Not wanted now:
- a reboot client;
- wider scopes.

`effector485` gets a word in `link.conf` for each power beyond files: `run` for starting programs. Without that word, RUN is refused with DENIED.
