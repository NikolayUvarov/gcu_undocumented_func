# 800-LNK-0015 — The shell's commands over the link, in developer mode

**Type:** service, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-10-22) · **Blocked by:** `KRN`: a client of the shell for `link` with `dev`; `APP`: what the shell's `run` takes from that client (both asked on `internal-dev-messages`, 2026-10-22) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-3.11, MC-12.1, MC-12.3

## Problem

On 2026-10-21 APP asked for the MacBook Pro's `ip`, `netstat -i`, `ps` and `dmesg`, to find why `wm` read `NET: NO CARD`. The link could not give them.
- **What the developer mode reaches:** `link` runs programs by name and reads the system log (800-LNK-0012).
- **What it does not reach:** `ip`, `ps`, `netstat` and the rest are the shell's own commands, and `link` holds no client of the shell. `mindlink run ip` answers "no such file".
- **The maintainer's word, 2026-10-22:** "you must be able to run commands there". An agent at the other end of the cable needs the shell's commands to debug the machine without a person.

## Plan

The shell already runs a client's command line: `idl/shell.wit` `run(line) -> bytes<7000>`, with `shell/src/clients.rs` deciding what it takes.

1. **`KRN`:** with `dev` in `link.conf`, `init` gives `link` a client of the shell, badged so the shell knows it is the link's.
2. **`APP`:** what `run` takes from that client.
   - Today, from any client, the information commands (`ps`, `ip`, `net`, …) run at once.
   - The rest waits for the user in the shell's window, and over the cable no one answers there.
   - The proposal: the commands that only read run at once for the link's client.
   - **The maintainer's choice, 2026-10-22, (a):** the commands that change the machine run at once too.
   - `netpolicy` with a change, `netrevoke` and `mode` stay refused over the link.
3. **`LNK`, this task:**
   - **`linkproto`:** COMMAND (54) on the programs channel. It carries a line of up to 240 bytes. The answer is a run whose output and end are read with OUTPUT (51) as a program's are: state ENDED with exit 0, or exit 1 with the shell's reason (`refused`, `declined`, `busy`, `unavailable`).
   - **`link`:** COMMAND calls the shell's `run` and keeps the answer as a run's output. It is logged as `[LINK] COMMAND <word>` with the command's word, not its line, so that no argument reaches the log.
   - **`mindlink sh "<line>"`:** prints the output, exits 0 or 1. **`effector485`:** a kind `shell`.
   - **`mindlink serve --run`** answers COMMAND with `sh -c`, for the host tests.

## Acceptance criteria

- [x] **Host tests:** COMMAND through the lossy line (`linkproto`); `mindlink sh` against `serve --run`; `effector485`'s `shell` (2026-10-22, below).
- [ ] **The QEMU `link` suite with `dev`:**
  - `mindlink sh ip` shows the cards;
  - `mindlink sh ps` shows `link`;
  - a command the link's client may not run is refused with its reason;
  - without `dev`, COMMAND answers UNSUPPORTED.
- [ ] **On the MacBook Pro over RS-485:** `ip`, `netstat -i`, `ps` and `dmesg -s netstack` read from the development machine.

Evidence for the configurations named only (MC-12.1). What the link's client may run is the shell's decision. This task claims no more than the shell grants it (MC-3.11, MC-12.3).

## Progress

- **2026-10-22, the protocol and the host's side, written; their host tests pass.**
  - **`linkproto`:** COMMAND (54), `Client::command`, and `Files::command`, UNSUPPORTED by default.
    - `shell_commands_over_a_lossy_line`: twenty lines through a line that loses, repeats and echoes frames, each run once;
    - the shell's refusal arrives as exit 1 with its reason;
    - an empty line, one over 240 bytes and one with a control character are refused.
  - **`mindlink sh <line>`:** its exit code is the line's. `serve --run` answers COMMAND with `sh -c`, and without `--run` refuses it.
  - **`effector485`'s kind `shell`:** followed as a program is, in the batch test.
  - **Docs:** `docs/link/protocol.md`, `mindlink.md` and `effector485.md`, English and Russian.
- **`link` on MIND Core** answers COMMAND with UNSUPPORTED until `init` gives it a client of the shell (`KRN`) and the shell takes the link's lines (`APP`).

## Related

[800-LNK-0012](../issues-done/800-LNK-0012-the-developer-mode.done), [800-LNK-0009](800-LNK-0009-programs-started-for-the-master.md), [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md), `idl/shell.wit`, `shell/src/clients.rs`, [docs/link/protocol.md](../docs/link/protocol.md).
