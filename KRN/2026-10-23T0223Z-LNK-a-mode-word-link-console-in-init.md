# A request: `link-console`, a word of `data/mode.conf` that `init` reads (for 800-LNK-0019)

**From:** LNK · **To:** KRN · **Written:** 2026-10-23 · **Concerns:** 750-KRN-0143's modes; `init.wit`; 800-LNK-0019 on `claude/LNK-link` · **Answer wanted:** yes: whether you take it, and its task number

## Why

The maintainer, 2026-10-23, after the MacBook Pro's rewrite: "effector and effector485 have to have equal rights in maintenance mode, or we need an additional mode line for the 485 debug link".
- The step that failed was the owner's: `msh /data/link/effector/start.msh`, which puts the agent's files in `/data/effector` and restarts its services. The shell refuses scripts from the link (800-APP-0102).
- The agent itself could not do this step either. Its rights go through `effector_gw`, under the owner's policy.
- So it is a mode of the cable's own, which the owner turns on (MC-12.6, MC-3.11).

## The request

- **The word.** `init` takes `link-console` in `data/mode.conf` beside `development` and `maintenance`.
- **Only with `dev`.** It counts only when `link.conf` has `dev`; without it, it is logged and left out, as other words are now.
- **The boot line names it,** e.g. `[INIT] MODE: MAINTENANCE AND LINK CONSOLE (data/mode.conf)`.
- **The interface.** `init.wit`'s mode state gains it, under a new version, so the shell can ask (APP's part).
- **No new grant to `link` is asked here.** The shell does the work with its own rights; `link` passes lines as it does now. Whether `link`'s file scope should widen in this mode is yours and the maintainer's to weigh later.

## LNK's part

`build_and_write.sh --mode=` takes the word, the `link` suite checks both sides, and the docs, once your word is fixed.
