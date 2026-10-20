# 351-APP-0083 — The shell lends a TLS client that cannot present the device certificate

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (on the tools branch, 2026-10-20; the gate left) · **Blocked by:** — (the right to badge `SLOT_TLS` is in `main` since ABI 10, 000-KRN-0109) · **Main task:** [351](351-self-update.md) · **Roadmap:** track D · **Constitution:** MC-3.11, MC-11.4

**Recorded by:** the network track (`NET`), 2026-10-12, in `requests-APP.md`, for 351-NET-0022. Numbered by the tools track on 2026-10-20, at the maintainer's word.

## Problem

`tls` refuses `client-certificate` to a client badged `BADGE_TLS_NO_DEVICE` (`mind::network`, `idl/tls.wit` 1.2). The shell lends its own unbadged client today, so a program can still sign in as the device.

## Plan

- For `REQUEST_TLS`, the shell lends a copy of `SLOT_TLS` minted with `BADGE_TLS_NO_DEVICE`, with the program's flow grant as before.
- It keeps the unbadged one for its own `https -c`.
- `docs/tools` (EN, RU) says which TLS client a program gets.

## Acceptance criteria

- **The `tls` suite:**
  - a program with the lent client gets `denied` for `client-certificate`;
  - the shell's `https -c` still sends the certificate.
- **`caps <pid>`** shows the lent client's badge.

## Progress (2026-10-20)

- **`shell/src/main.rs`, `start`:** for `REQUEST_TLS` with a flow grant the shell mints a copy of `SLOT_TLS` with `BADGE_TLS_NO_DEVICE` and only `CAP_WRITE | CAP_GRANT`, lends it in the program's `SLOT_TLS` and drops its own copy of it. Its unbadged client stays its own, for `https -c`.
- **`docs/msh.md`** (EN, RU): a script's program gets a TLS client that cannot offer the device certificate.
- **The `tls` suite's lending check** (`tls_lending_check`): the client in `tlsclock`'s slot 20 is `tls`'s with `BADGE=1`; `https -c` still sends the device certificate (the suite's own check).
- **Not checked here:** a `client-certificate` call from a program, refused by `tls` for the badge (351-NET-0022, the network track's); no program the tools track has makes one.
- **Left:** the gate on this batch.

## Related

351-NET-0022 (`BADGE_TLS_NO_DEVICE`), [000-KRN-0109](../issues-done/000-KRN-0109-the-shells-clients-may-badge-for-programs.done) (`SLOT_TLS` with `CAP_BADGE`).
