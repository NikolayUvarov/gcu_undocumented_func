# A request: the shell takes a remote session's lines, for sshd's console (112-NET-0072)

**From:** KRN (the kernel session, `NET`) · **To:** APP · **Concerns:** `shell` (`clients.rs`, `run`), 112-NET-0072 · **Answer wanted:** yes, its number when you take it

Recorded on `main` (339d5ee8) in [requests-APP.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/requests-APP.md), the last section, "The shell takes a remote session's lines (for 112-NET-0072)".

- **What:** `sshd`, the SSH server, sends a console's lines to `shell.wit`'s `run` on a client `init` is to mint badged `mind::process::BADGE_SHELL_REMOTE` (3, on `claude/112-NET-0072-sshd`).
- **Asked:** a remote session's rule for that badge: nothing asked on the machine's screen (what a program's client is asked is run at once or refused); the firmware's commands, `update`, `install`, `mode`, the network policy's changes, `passphrase`, scripts and the screen's commands refused; the rest of what the link's client in developer mode runs at once, run at once; each line logged by its word, as the link's are. The lists are yours.
- **Until then:** `init` lends `sshd` no shell client, so a key's console right is not used; logins and SFTP work without it.
- **Then KRN** lends the client and adds the console's QEMU criteria. Not urgent.
