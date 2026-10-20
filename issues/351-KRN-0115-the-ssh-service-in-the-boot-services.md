# 351-KRN-0115 — The SSH service in the boot services: `ssh` started by `init`, `keystore`'s client with `BADGE_KEY_SSH`, `SLOT_SSH` and `REQUEST_SSH`

**Type:** `init`, ABI (`common/abi.rs`, ABI 12, with 351-KRN-0116), tests · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made, ABI 12; the gate left) · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-2.3, MC-3.4, MC-11.9

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
5. **ABI 12** goes with `libmind`, `docs/api`, the README's slot table and `check`'s `launch: every slot`. It moves `SLOT_DYNAMIC` up.

## Acceptance criteria

- [ ] In the `net` suite (x86 and aarch64), `ssh` runs and holds `keystore`'s client with `BADGE_KEY_SSH` (`stat caps`), and the shell's client is in `SLOT_SSH`.
- [ ] `check`'s `launch: every slot` lends an endpoint in every launch slot, `SLOT_SSH` among them.
- [ ] Every other suite passes as before. The SFTP check of the `net` suite comes from the network track.

## Progress (2026-10-20)

Made, with ABI 12:
- **Boot image.** `ssh` (`ssh.elf`) is a boot image after `tls` (`BOOT_IMAGES` 39).
- **`init`** starts it with:
  - its server endpoint;
  - `keystore`'s client badged `BADGE_KEY_SSH` in slot 4;
  - the log client;
  - no network, as `tls`: clients lend their flows, which is how the service is written (351-NET-0027).
- **The shell** holds its client in `SLOT_SSH` (34) with `CAP_BADGE`.
- **For programs.** `REQUEST_SSH` (`1 << 24`) is defined and `SLOT_SSH` is a launch slot. The shell's lending for `REQUEST_SSH` is the tools track's part (requests-APP.md).
- **Docs.** README, docs/api, the profile and `svc`'s table are updated.

Tests:
- **The `tls` suite**, which already examines the key service's clients, requires:
  - `ssh` holds `keystore`'s client with badge 3;
  - the shell's slot 34 names `ssh`'s endpoint.
- **`check`'s "launch: every slot"** lends 20 slots, `SLOT_SSH` among them.

Both pass locally on x86 (QEMU), with `bench`. Left: the gate, x86 and aarch64.

## Related

[351-NET-0027](351-NET-0027-the-ssh-service.md), [351-NET-0026](../issues-done/351-NET-0026-the-ssh-protocol.done), [000-KRN-0109](../issues-done/000-KRN-0109-the-shells-clients-may-badge-for-programs.done), [351-KRN-0116](351-KRN-0116-the-shells-client-of-the-updater.md), the APP request "the shell lending the SSH service's client" in [requests-APP.md](requests-APP.md).
