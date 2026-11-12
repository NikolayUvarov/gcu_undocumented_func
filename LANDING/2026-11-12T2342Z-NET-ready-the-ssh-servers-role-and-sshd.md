# Ready: the SSH server's role in ssh/ and sshd, the SSH server as a MIND Core service

**From:** NET (the kernel session) · **To:** LANDING · **Written:** 2026-11-12 23:42 UTC

**Ready:** `claude/112-NET-0072-sshd` d4e6499b81

- **Advances:** [112-NET-0071](../../main/issues/112-NET-0071-the-ssh-servers-role-in-ssh.md) (`ssh`'s server role behind the feature `server`, host-tested against OpenSSH's client and fuzzed) and [112-NET-0072](../../main/issues/112-NET-0072-sshd-as-a-mind-core-service.md) (`sshd`: `keystore.wit` 1.8 `ssh-host`, `ssh::keys`, `init`'s grants with the shell's client badged `BADGE_SHELL_REMOTE`, **handoff 34**: 51 boot images). The `KRN` part (`common/abi.rs`, `init`) is mine.
- **Merged:** `main` 60b995bd (112-APP-0136 in it), with no work of another track that has not landed.
- **Tested:**
  - host: `ssh`'s `server_openssh`, `server_fuzz`, `keys`; `boot_abi`, `services`, `clipboard`; the IDL test;
  - QEMU in this container, x86: `--suites sshd` with the console (`ps` run by the shell, `update` refused);
  - the maintainer's machines at d95c55c5: "x86: network, TLS and Effector" on x32 (1357 s) and "aarch64: network, TLS and Effector" on acc-1 (1145 s), both passed.
- **For APP:** `mind::process::BADGE_SHELL_REMOTE` (3) arrives with this; `clients::BADGE_REMOTE` may take it.
