# 351-APP-0084 — The shell lends a client of the SSH service

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (planned) · **Blocked by:** — (`SLOT_SSH` and `REQUEST_SSH` are in `main`, 351-KRN-0115) · **Main task:** [351](351-update-and-provenance.md) · **Roadmap:** track D · **Constitution:** MC-3.11, MC-11.4

**Recorded by:** the network track (`NET`), 2026-10-14, in `requests-APP.md`, for [351-NET-0027](351-NET-0027-the-ssh-service.md). Numbered by the tools track on 2026-10-20, at the maintainer's word.

## Problem

The service `ssh` reads files over SFTP for programs, logged in as the device with a key that only `keystore` holds. A program reaches it as it reaches `tls`: through a client the shell lends. The shell does not lend it yet.

## Plan

- For `REQUEST_SSH` the shell lends a copy of its `SLOT_SSH` client in the program's same slot, with the program's flow grant, as it does for `REQUEST_TLS`.
- A script declares it as `requires: ssh`.
- `docs/tools` (EN, RU).

## Acceptance criteria

- **The `net` suite:** `download sftp://…` started from the shell reads a file from the host's OpenSSH server through the lent client.
- **Without the service** the program is told so, and starts.

## Related

[351-NET-0027](351-NET-0027-the-ssh-service.md) (the service), 351-KRN-0115 (`SLOT_SSH`, `REQUEST_SSH`).
