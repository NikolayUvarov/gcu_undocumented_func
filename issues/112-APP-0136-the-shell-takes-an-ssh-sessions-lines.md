# 112-APP-0136 — The shell takes an SSH session's lines

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — (the client `init` mints for `sshd`: KRN's, after 112-NET-0072) · **Main task:** [112](112-our-own-ssh-server.md) · **Roadmap:** track D · **Constitution:** MC-3.1, MC-5.1, MC-11.4, MC-11.6

**Asked by:** KRN (the kernel session, `NET`), for 112-NET-0072's console, in [requests-APP.md](requests-APP.md), 2026-11-12. Numbered on 2026-11-12.

## Problem

`sshd` (112-NET-0072) sends each line of a login's console to `shell.wit`'s `run`, on a client `init` is to mint badged `mind::process::BADGE_SHELL_REMOTE` (3). The shell knows two rules for `run`: a program's, which asks the user in the shell's window for what changes the machine, and the link's in developer mode, which runs that at once. A line typed somewhere else must never be asked on the machine's screen: nobody may be there, and a person there must not agree to what someone elsewhere typed.

## Plan

- **A rule of its own** for a client badged `BADGE_SHELL_REMOTE`, in `shell/src/clients.rs` (`Client::Remote`):
  - **run at once:** what the link's client in developer mode runs at once (`ps`, `kill`, `logs`, `reboot`, `date set`, `hostname <name>`, …);
  - **refused** as for the link: a change of the network policy (`netpolicy` with a change, `netrevoke`), the machine's modes, scripts (`msh`), the screen's commands, programs and statements, and every command no client gets (`update`, `passphrase`, `boot`, …);
  - **refused besides:** a line whose output goes into a file (`>`, `>>`), as files are the key's `files` right's, through SFTP; and `tpm`, the machine's own root of trust.
- **Never asked:** a remote line's question on the screen is answered no and logged, whatever command asks it.
- **`run` alone:** the copy badged for a remote session reaches no other call of `shell.wit` (a window, a setting, a lent client), and is refused with a line in the log.
- **Logged** by the command's word alone, as the link's: `[SHELL] FOR AN SSH SESSION THE SHELL RAN PS`.
- The badge's value is named in the shell until 112-NET-0072 brings `mind::process::BADGE_SHELL_REMOTE` to `main`; then the shell takes that one.

## Acceptance criteria

- **Host tests:** the remote rule's lists, beside the link's.
- **QEMU (`tls` suite's `sshd_check`, KRN's after `init` lends the client):** `ssh mind@… ps` answers the shell's `ps`; a refused command says so; the log names its word.

## Related

[112](112-our-own-ssh-server.md), [requests-APP.md](requests-APP.md), 800-APP-0102 (the link's client), 800-APP-0108 (the link's console).
