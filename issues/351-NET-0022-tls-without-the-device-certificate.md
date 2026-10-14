# 351-NET-0022 — A TLS client that cannot present the device certificate

**Type:** network (service, interface) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P2 · **Status:** open · **Blocked by:** — for the service; `KRN` and `APP` for lending such a client ([requests-KRN.md](requests-KRN.md), [requests-APP.md](requests-APP.md)) · **Main task:** [351](351-self-update.md) · **Roadmap:** track D · **Constitution:** MC-3.11, MC-11.9

From the tools track's request in requests-NET.md ("A TLS client without the device certificate", 2026-10-09).

## Problem

The shell lends its client of `tls` to a program that asks for `REQUEST_TLS` and gets a flow grant (351-APP-0017). With it, the program may `connect` with `client-certificate` and so sign in as the device to any server its grant reaches. Fetching a file over HTTPS needs no such thing.

## Plan

- **The restriction is the badge.** A client of the TLS service with `BADGE_TLS_NO_DEVICE` (`mind::network`, `idl/tls.wit` 1.2) is refused `client-certificate` (`denied`, logged). An unbadged client keeps it, so the shell's own `https -c` and every client today work as before.
- **Lending such a client:**
  - the shell needs the right to badge a copy of its client (`CAP_BADGE` on its `SLOT_TLS`), which `init` gives (`KRN`);
  - the shell then lends the badged copy to programs (`APP`).
- **The updater.** `init`'s client for `updater` (351-KRN-0022) could be the badged kind too, unless the update server asks for the device's certificate. That is the kernel track's choice.

## Acceptance criteria

- `tls` refuses `client-certificate` to a badged client and logs it; it still answers an unbadged one. Built for x86 now.
- **In QEMU** (the `tls` suite), once the shell lends the badged copy: a program with the lent client gets `denied` for `client-certificate`, while the shell's `https -c` still sends the certificate.

## Related

[351-NET-0002](../issues-done/351-NET-0002-https-for-programs.done), [351-APP-0017](../issues-done/351-APP-0017-shell-lends-the-tls-client.done), [docs/network/services.md](../docs/network/services.md).
