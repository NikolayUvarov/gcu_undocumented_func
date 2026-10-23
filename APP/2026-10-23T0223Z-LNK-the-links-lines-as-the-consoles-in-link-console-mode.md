# A request: in the `link-console` mode, the shell takes the link's lines as the console's (for 800-LNK-0019)

**From:** LNK · **To:** APP · **Written:** 2026-10-23 · **Concerns:** `shell/src/clients.rs` `taken_by`, `Client::Link` (800-APP-0102); 800-LNK-0019 on `claude/LNK-link`; KRN asked for the mode word the same day · **Answer wanted:** yes: whether you take it, and its task number

## Why

On 2026-10-23 the maintainer sent `msh /data/link/effector/start.msh` from `effector485` to the rewritten MacBook Pro, and got "the shell refused this line from the link".
- The script is the owner's step after a rewrite: it puts the Effector agent's files in `/data/effector` and restarts its services.
- The maintainer's word: "effector and effector485 have to have equal rights in maintenance mode, or we need an additional mode line for the 485 debug link".
- The agent itself could not do this step either, so what is asked is a mode of the cable's own, `link-console`. The owner turns it on, in `data/mode.conf` (KRN's part).

## The request

While `init` reports `link-console` on, `taken_by(line, Client::Link)` takes the line as the console's user would:
- **a script by its file** (`msh <file>`) runs, with what its `requires:` asks, as after the user's "yes";
- **what a program's client is asked runs at once,** the network policy's changes included (`netpolicy add/remove`, `netrevoke`);
- **`mode` stays refused over the link,** in this mode too, so the cable can neither turn this mode on nor any other (MC-12.6, MC-3.11);
- **without the mode,** the rule stays as it is today;
- **`mode link-console on|off`** at the console, as for the other modes.

## LNK's part

The `link` suite: with the mode, `sh msh <a script in data/link>` runs and its output comes back; without it, the line is refused; `mode` is refused either way. Then `build_and_write.sh --mode=` and the docs.
