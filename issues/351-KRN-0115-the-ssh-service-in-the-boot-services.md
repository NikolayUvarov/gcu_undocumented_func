# 351-KRN-0115 — The SSH service in the boot services: `ssh` started by `init`, `keystore`'s client with `BADGE_KEY_SSH`, `SLOT_SSH` and `REQUEST_SSH`

**Type:** `init`, ABI (`common/abi.rs`, ABI 11, with 351-KRN-0116), tests · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-2.3, MC-3.4, MC-11.9

## Problem

The network track's `ssh` service ([351-NET-0027](351-NET-0027-the-ssh-service.md)) reads release files over SFTP for programs, as `tls` runs HTTPS for them. Its logins are signed by `keystore`, so the device key never leaves it (MC-11.9). Only `init` can start it and hand out its clients, and no boot image, slot or request flag names it yet. The network track's request is in [requests-KRN.md](requests-KRN.md) of 2026-10-14.

## Plan

1. **A boot image.** `ssh` (`ssh.elf`, from `ssh_service/`) joins `BOOT_SERVICES` after `tls`, with the build, the signed manifest and the suites' service list.
2. **`init` starts it** after `keystore` and the network stack, as it starts `tls`, and restarts it the same way.
3. **Its grants:**
   - a client of `keystore` with `BADGE_KEY_SSH` (`mind::network`, value 3) in slot 4, as `tls` holds `BADGE_KEY_SIGNER`; nothing else gets that badge;
   - the log client;
   - the network stack's client and a socket grant, as `tls` has them.
4. **Its clients:**
   - the shell's, in a new fixed slot `SLOT_SSH`, with `CAP_BADGE` as `SLOT_TLS` (000-KRN-0109);
   - for programs, `REQUEST_SSH` lends the launcher's client in the same slot, for a program that also gets a flow grant, as `REQUEST_TLS` does. `SLOT_SSH` joins `LAUNCH_SLOTS`.
5. **ABI 11** goes with `libmind`, `docs/api`, the README's slot table and `check`'s `launch: every slot`. It moves `SLOT_DYNAMIC` up.

## Acceptance criteria

- [ ] In the `net` suite (x86 and aarch64), `ssh` runs and holds `keystore`'s client with `BADGE_KEY_SSH` (`stat caps`), and the shell's client is in `SLOT_SSH`.
- [ ] `check`'s `launch: every slot` lends an endpoint in every launch slot, `SLOT_SSH` among them.
- [ ] Every other suite passes as before. The SFTP check of the `net` suite comes from the network track.

## Related

[351-NET-0027](351-NET-0027-the-ssh-service.md), [351-NET-0026](../issues-done/351-NET-0026-the-ssh-protocol.done), [000-KRN-0109](000-KRN-0109-the-shells-clients-may-badge-for-programs.md), [351-KRN-0116](351-KRN-0116-the-shells-client-of-the-updater.md), the APP request "the shell lending the SSH service's client" in [requests-APP.md](requests-APP.md).
