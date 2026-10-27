# 800-APP-0108 — In the `link-console` mode, the shell takes the link's lines as the console's

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P1 (the maintainer: "effector and effector485 have to have equal rights in maintenance mode, or we need an additional mode line for the 485 debug link", 2026-10-23) · **Status:** open · **Blocked by:** [800-KRN-0155](../issues-done/800-KRN-0155-the-links-console-mode.done) in `main` (`init.wit` 1.8 `link-console`; on `claude/youthful-mendel-mf1soy` c10b15b5, in the landing queue for round 16) · **Main task:** [800](800-mind-link.md) · **Roadmap:** the development of MIND Core on real hardware · **Constitution:** MC-3.11, MC-12.3, MC-12.6

**Recorded by:** the tools track, 2026-10-23, from LNK's request (`APP/2026-10-23T0223Z-LNK-the-links-lines-as-the-consoles-in-link-console-mode.md` on `internal-dev-messages`), for 800-LNK-0019.

## Problem

- **What failed.** On 2026-10-23 the maintainer sent `msh /data/link/effector/start.msh` from `effector485` to the rewritten MacBook Pro. The shell answered that it refused this line from the link.
  - The script is the owner's step after a rewrite: it puts the Effector agent's files in `/data/effector` and restarts its services.
- **Why.** `clients::taken_by` gives the link's client (`Client::Link`, 800-APP-0102) at once what a program's client is asked. It never gives it a script, the network policy's changes or `mode`.
- **What is asked.** A mode of the cable's own, `link-console`, which the owner turns on in `data/mode.conf`. KRN's part is 800-KRN-0155: `init` counts the word only with `dev` in `link.conf`, and `init.wit` 1.8 `link-console()` tells it.

## Plan

- **The shell asks `init`** `link-console()` (`init.wit` 1.8) when a line comes from the link. An older `init`, or an error, counts as off.
- **While it is on,** `taken_by(line, Client::Link)` takes the line as the console's user would after a "yes":
  - a script by its file (`msh <file>`) runs, with what its `requires:` asks, the questions answered yes;
  - the network policy's changes run at once (`netpolicy add/remove`, `netrevoke`).
- **What stays refused over the link, in this mode too:**
  - `mode`, so that the cable can turn neither this mode nor any other on (MC-12.6, MC-3.11);
  - `msh -c` code and options, as for every client.
- **Without the mode,** the rule stays as it is today.
- **Seen.** The system log names each line the link ran in this mode: `[SHELL] LINK CONSOLE: <line>`. While the mode is on, the shell's notice above the first prompt says so.
- **`mode`** shows the state at the console: `LINK CONSOLE: ON (data/mode.conf, dev in link.conf)`.
- **Not `mode link-console on|off`.** LNK asked for it, but by `init.wit` 1.8 only the disk's file sets the word, and `set-mode` keeps it. The shell does not offer what `init` refuses. If the owner wants it from the console, that is a request to `KRN` for a `set-mode` of it.

## Acceptance criteria

- **Host tests** (`tests/shell_host.rs`): `taken_by` with and without the mode, for scripts, the policy's changes, `mode`, `msh -c`, and every other word as before.
- **The QEMU `link` suite** (LNK's part, 800-LNK-0019):
  - with the mode and `dev`, `sh msh <a script in data/link>` runs and its output comes back;
  - without it, the line is refused;
  - `mode` is refused either way.
- **Docs:** `docs/tools/README.md` and `README_RU.md`, beside 800-APP-0102's paragraph.

What the link may do in this mode is shown on the configurations tested only (MC-12.1). The mode gives the cable the console's rights: whoever holds the cable then acts as the console's user.

## Related

- [800-APP-0102](800-APP-0102-the-shells-run-for-the-links-client.md): the link's client.
- 800-LNK-0019: the link's part.
- [750-APP-0105](../issues-done/750-APP-0105-the-mode-command.done): `mode`.
