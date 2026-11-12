# 112-NET-0072 — `sshd` as a MIND Core service: logins with the owner's keys, SFTP into `data/`, the host key in `keystore`

**Type:** network (a new service `sshd/`, `keystore` 1.8, `ssh` `keys`, the grants in `init` as a `KRN` part) · **Owner:** `NET` · **Priority:** P2 · **Status:** in progress · **Blocked by:** — (the console: the request in [requests-APP.md](requests-APP.md), "The shell takes a remote session's lines") · **Main task:** [112](112-our-own-ssh-server.md) (stage 1) · **Roadmap:** track D · **Constitution:** MC-3.4, MC-5.1, MC-5.2, MC-10.3, MC-11.4, MC-11.6, MC-11.9, MC-11.11, MC-12.3

## Problem

[112-NET-0071](112-NET-0071-the-ssh-servers-role-in-ssh.md) made the protocol's server role, host-tested against OpenSSH's client. Nothing runs it on a MIND machine: no service listens on TCP 22, no host key, no list of the owner's keys.

## Plan

1. **The host key is the device key.** `keystore.wit` 1.8 adds the purpose `ssh-host`: an SSH key exchange's hash, exactly 32 bytes, signed for the SSH server's badge (`BADGE_KEY_SSH_HOST`) alone, at most 60 a minute. No other form the device key signs is 32 bytes long, so such a signature counts for no TLS handshake and no SSH login. The key never leaves `keystore` (MC-11.9).
2. **The owner's keys: `data/ssh/authorized_keys`,** read at each connection (`ssh::keys`, host-tested). Each line names what its key may do before the key: `console`, `files` or `console,files`. A line that names no right, another key type, a damaged key or another word is left out, and the log says which and why. The user is `mind`.
3. **The service `sshd/`:**
   - **Off by default.** `init` starts it only when the policy gives program `sshd` a flow, whose listening grant must name TCP 22 (MC-11.6). It listens only once the keys file lists a key.
   - **Its grants:** that flow; `keystore`'s client badged for `ssh-host`; a writable scope of `data/`. Nothing else (MC-11.4).
   - **SFTP reaches `data/` but `data/ssh/`,** refused as FAT names it (any case, trailing dots and spaces dropped), so a key with the files' right never reaches another key (MC-3.4).
   - **Limits:** one session at a time; a login within 30 s; 10 min of silence at most; 10 connections a minute from one address; 1 GiB written a session; 8 open files.
   - **The log** (MC-10.3): each login with its key's fingerprint (as `ssh-keygen -l` prints it) and comment, each refusal and why, each console command's word, each file written or removed, each session's end with the bytes written. No session's data.
4. **The console** runs the shell's commands through its client in `SLOT_SHELL`, with a remote session's rule (the shell's to set: nothing that asks on the machine's screen, no firmware, no update, no network policy, MC-3.11). `init` lends that client only once the shell takes such a client's lines on that rule (the request in `requests-APP.md`). Until then a key's console right is not used, and the log says so at its login.

## Acceptance criteria

- **Host:** `ssh/tests/keys.rs`: the keys file's lines taken and left out; base64 as OpenSSH writes it; the fingerprint as `ssh-keygen` prints it; the keys' directory as FAT names it.
- **QEMU, x86 and aarch64,** the policy holding `listen sshd tcp 22 from 10.0.2.0/24`, OpenSSH's client on the host through QEMU's forwarded port:
  - the host key checked against a known_hosts line made of the device key `keystore` logs;
  - `scp` copies a file into `data/` and back, with equal SHA-256; `sftp` makes, lists, renames and removes;
  - `data/ssh/` and a path above the root are refused;
  - an unlisted key and a password are refused;
  - each login, refusal and write is logged; `keystore` logs `SIGNED SshHost`.
- **Without the grant:** `sshd` is not started, and `init` says why.
- **The console** (with 112-APP-0136): the console key's `ssh … ps` lists the processes through the shell, `update` is refused, the files' key gets no console; the shell logs both lines.
- **The profile** says what `sshd` is trusted with: the host key's use, the files' scope, the console's authority.

## Progress

**Made (2026-11-12)** on `claude/112-NET-0072-sshd`, on 110-NET-0069/0070's branch (`keystore.wit` 1.7, handoff 33) and 112-NET-0071's:
- **`keystore.wit` 1.8 `ssh-host`** and `mind::network::BADGE_KEY_SSH_HOST` (7); `keystore` signs 32 bytes alone for that badge, 60 a minute.
- **`ssh/src/keys.rs`:** the keys file's lines, base64, OpenSSH's fingerprint, and `under_dir`, the keys' directory as FAT names it; `ssh/tests/keys.rs` (4 tests), in `scripts/host_tests.sh`.
- **`sshd/`:** the service as planned. Its console sends lines to `shell.wit`'s `run` in `SLOT_SHELL` when it holds a client there; `init` lends none yet.
- **`mind::process::BADGE_SHELL_REMOTE`** (3), for the client the shell is asked to take (requests-APP.md, 339d5ee8 on `main`).
- **`init`** (the `KRN` part): `sshd` after `httpd`, last before the shell, **handoff 34** (`common/abi.rs`, `tests/boot_abi_host.rs`: 51 boot images, BootInfo 1176 bytes); its grants; not started without the policy's flow (`THE POLICY GIVES IT NO LISTENING GRANT`); `NEEDS` on `netpolicy` and `keystore`.
- **`tests/qemu_smoke.py`:** `sshd_check`, at the end of the `tls` suite and alone as `--suites sshd`; the `tls` suite's first boot checks `init`'s line without the grant.
- **Docs:** `docs/network/services.md` (the SSH server), the profile's MC-11.9 row, `tcb.md`, `bootstrap.md`, the IDL table.
- **Tested** (this container, x86, QEMU 8.2.2 under TCG, OpenSSH 9.6p1's client): `--suites sshd` passed: the host key matched the device key's known_hosts line and fingerprint; scp up and down 300 000 bytes with equal SHA-256; sftp's mkdir, put, ls, rename, rm and rmdir; `data/ssh/`, `/SSH/` and `../../x` refused, and scp into `data/ssh/`; an unlisted key and a password refused with `Permission denied (publickey)`; `ps` refused to both keys (no console lent); the log lines; `SIGNED SshHost`. The host tests the change touches pass (`keys`, `server_openssh`, `server_fuzz`, `boot_abi`, `services`, `idl_fuzz`).
- **The console lent** (2026-11-12, after 112-APP-0136, APP's answer `KRN/2026-11-12T2125Z`): `init` gives `sshd` the shell's client badged `BADGE_SHELL_REMOTE` in `SLOT_SHELL`; `sshd_check` runs `ps` and `update` through the console key. Docs: `docs/network/services.md`, `tcb.md`, `bootstrap.md`. Tested (this container, x86, QEMU under TCG, on this branch with APP's 6159739a merged in a throwaway tree, not pushed): `--suites sshd` passed, the console key's `ps` listed the processes through the shell and `update` was refused, both logged by `sshd` and the shell. Its first run found that the suite's 11th connection within a minute met `sshd`'s limit of 10 from one address (the files' part makes 7): the suite now waits out the minute before the last 5.
- **Left:** `--suites sshd` once `main` holds 112-APP-0136; the `tls` suite on x86 and aarch64 on the maintainer's machines; the gate.

## Related

- [112](112-our-own-ssh-server.md): the main task.
- [112-NET-0071](112-NET-0071-the-ssh-servers-role-in-ssh.md): the protocol's server role.
- [110-NET-0070](110-NET-0070-httpd-as-a-mind-core-service.md): `httpd`, the other listening service, started the same way.
