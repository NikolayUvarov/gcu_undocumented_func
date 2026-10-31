# 112 — Our own SSH server, `sshd`: logins into a MIND machine and file copy (SFTP, `scp`)

**Type:** network (main task: a server) · **Owner:** `NET` (the kernel session) · **Priority:** P2 · **Status:** open · **Blocked by:** [110-NET-0052](110-NET-0052-listening-tcp-sockets.md) (listening sockets and the listening grant) · **Roadmap:** track D (network) · **Constitution:** MC-3.1, MC-5.1, MC-5.2, MC-10.3, MC-11.4, MC-11.6, MC-11.9, MC-11.11, MC-12.3

**Recorded by:** the kernel session, 2026-10-31, at the maintainer's request: "check whether we have tasks for our own HTTP(S) server and our own SSH (scp) server; if not, create them." The HTTP(S) server is [110](110-our-own-web-server.md). This is the SSH one.

## Problem

- **What SSH does today is the client's side only:**
  - `ssh/` is an SSH 2 client (351-NET-0026);
  - the `ssh` service logs in as the device with the device key, which `keystore` signs for (351-NET-0027);
  - `download sftp://` reads a file from another machine.
- **Nothing can log into a MIND machine over the network,** or copy a file to or from it. The ways in today:
  - the link over a USB cable, in developer mode (800-LNK-0012);
  - the Effector agent, in a mode the owner set (750-EFF-0013).

  Neither is a standard tool that any `ssh` or `scp` reaches.
- **Nothing on MIND Core accepts a connection yet** (110-NET-0052).

## Plan

### Stage 1: logins and SFTP

- **The protocol's server role** in `ssh/`, next to its client, with the same choices:
  - `curve25519-sha256` with OpenSSH's strict key exchange;
  - `chacha20-poly1305@openssh.com`;
  - an `ssh-ed25519` host key;
  - no compression, no passwords, no keyboard-interactive.

  Its parsing is host-tested and fuzzed, and runs in `sshd` alone, which holds no other authority (MC-11.11).
- **The host key lives in `keystore`** and never leaves it (MC-11.9). `keystore` signs only the exchange hash of a key exchange, for `sshd`'s badge.
- **Who may log in:** public keys only, from a file only the owner writes (`system/ssh/authorized_keys`).
  - Each key names what it may do: a console, file copy, or both, and the scope file copy reaches.
  - Attempts are limited by client address and in all, and each refusal is logged.
- **A login's console:** the shell's commands, as the link's console gives them (800-KRN-0155). A remote session gets no authority that asks the user on the machine's screen: firmware, update, network policy. Only the owner's policy may name one.
- **File copy: SFTP,** read and write within one VFS scope, `data/` by default, with the scope's quota.
  - OpenSSH's `scp` uses SFTP since version 9.0, so `scp` works with this.
  - The old SCP protocol (`scp -O`) is stage 2, if asked for.
- **Off by default.** `sshd` starts only with a listening grant for it (`listen sshd tcp 22 from <network>`) and an `authorized_keys` file. The networks it is reached from are the grant's (MC-11.6).
- **Limits** (MC-5.1, 5.2): connections at once and per client address; a deadline for the key exchange and the login; bytes a session may write.
- **The log** (MC-10.3): each login with the key's fingerprint and the client's address, each file written or removed, each session's end. No session's data is logged.

### Stage 2: later (intention only)

- The old SCP protocol, port forwarding (refused until asked for), `ssh-ed25519` certificates instead of a key file.

## Acceptance criteria

- **Host tests:** the server's state machine against `ssh/`'s own client and against OpenSSH's client (`ssh/tests/openssh.rs` does the converse today). Refusals: an unknown key, a password, a wrong signature, a channel the key may not open.
- **A fuzz target** for the server's packet parser (ASR's harness).
- **QEMU, x86 and aarch64,** with OpenSSH's client on the host through QEMU's forwarded port:
  - `ssh` logs in with a listed key and runs `ps`;
  - an unlisted key and a password are refused;
  - `scp` copies a file into `data/` and back, with equal SHA-256;
  - a path outside the scope is refused;
  - with no listening grant, nothing answers on port 22.
- **The profile** says what `sshd` is trusted with: the host key's use, the console's authority, the scope.

## Related

[110](110-our-own-web-server.md), [110-NET-0052](110-NET-0052-listening-tcp-sockets.md), [351-NET-0004](351-NET-0004-ssh-client.md), [351-NET-0026](../issues-done/351-NET-0026-the-ssh-protocol.done), [351-NET-0027](351-NET-0027-the-ssh-service.md), [800-KRN-0155](../issues-done/800-KRN-0155-the-links-console-mode.done), [000-ASR-0008](000-ASR-0008-test-only-openssh-path.md).
