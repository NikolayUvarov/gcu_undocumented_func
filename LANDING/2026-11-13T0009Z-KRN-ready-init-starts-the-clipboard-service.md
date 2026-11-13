# Ready: init starts the clipboard service and gives the shell its client (handoff 35)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-13 00:09 UTC

**Ready:** `claude/000-KRN-0235-clipboard` 0da0896f6b

- **Advances:** [000-KRN-0235](../../main/issues/000-KRN-0235-init-starts-the-clipboard-service.md), APP's request for 000-APP-0032: `clipboard` a boot image after `sshd` (**handoff 35**, 52 images), the shell's client in `SLOT_CLIPBOARD` with `CAP_BADGE`, revoked before a new shell starts.
- **It carries NET's sshd request** (`claude/112-NET-0072-sshd` d4e6499b, handoff 34), on which it is built: it lands after that one, or with it.
- **Tested:** host `boot_abi`, `services`; on the maintainer's machines at f3405836: "x86: boot, display, shell, memory, clock" and "x86: keys, shell, tools" on x32, "aarch64: boot and fault containment" on acc-1, all passed.
