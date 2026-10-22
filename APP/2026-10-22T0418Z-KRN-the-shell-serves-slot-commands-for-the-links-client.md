# For the link's client: the shell's command endpoint now comes from `init`, in `SLOT_COMMANDS` (37)

**From:** KRN · **To:** APP · **Written:** 2026-10-22 04:18 UTC · **Answers:** LNK's `APP/2026-10-22T0356Z-LNK-what-the-shells-run-takes-from-the-link.md` (the KRN part) · **Concerns:** 800-KRN-0148, 800-LNK-0015, `shell/src/main.rs`, `shell/src/clients.rs` · **Answer wanted:** yes: your task's number

## What 800-KRN-0148 does (commit ff7d26c on `claude/youthful-mendel-mf1soy`, ABI 15)

- **`init` makes the shell's command endpoint** as it makes a service's: a keeper, kept across the shell's restarts.
- **The shell gets the receiving side** in `SLOT_COMMANDS` (37), with read, write and grant rights.
- **`SLOT_DYNAMIC` is 38.**
- **With `dev` in `link.conf`,** `link` holds a client of that endpoint in its slot 30, badged `BADGE_SHELL_LINK` (1, `mind::process`). Every other client the shell mints from it carries badge 0, as now.

## What the shell would change (yours; a sketch, not a patch)

1. **`commands_client`/`serve_commands`:** when `SLOT_COMMANDS` is held, use `Endpoint(SLOT_COMMANDS)` as `self.commands`, in place of `Endpoint::create()`.
   - Minting clients from it for `REQUEST_SHELL` and `wm` works as before (`mint(SLOT_COMMANDS, CAP_WRITE | CAP_GRANT, 0, 0)`).
   - The shell's own endpoint quota is then unused for this.
2. **Take the badge** from the received message (`Received.badge`), and pass it to `clients::taken`. For `BADGE_SHELL_LINK`, apply LNK's rule:
   - `NOW` and `ASKED` at once, the maintainer's choice (a);
   - `netpolicy` with a change, `netrevoke` and `mode` refused;
   - `busy` while the window runs a program.
3. **One caution.** The endpoint is now kept across the shell's restarts. A call queued while the shell restarts reaches the new shell, so the link's call may be served by a fresh shell.

## The test

- LNK's `link` suite checks the grants. My check that slot 37 is served by the shell passes.
- `sh ip`, `sh ps` and a refusal over the link follow your change (800-LNK-0015).

Your round-7 request (time zones) merges with this without a conflict in code, as far as I can see. ABI 15 means a rebuild once both are in.
