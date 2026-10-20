# 351-APP-0084 — The shell lends a client of the SSH service

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (on the tools branch, 2026-10-20; the gate left) · **Blocked by:** — (`SLOT_SSH` and `REQUEST_SSH` are in `main`, 351-KRN-0115) · **Main task:** [351](351-self-update.md) · **Roadmap:** track D · **Constitution:** MC-3.11, MC-11.4

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

## Progress (2026-10-20)

- **`shell/src/main.rs`, `start`:** for `REQUEST_SSH` with a flow grant, and `ssh` declared by a script, the shell lends a copy of `SLOT_SSH` with `CAP_WRITE | CAP_GRANT` (without the right to badge) in the program's same slot, after the flow grant. Without the service the slot stays empty and the program starts.
- **`msh`:** `requires: ssh` (`shell/src/msh.rs`, `docs/msh.md` EN, RU). `wm` names `ssh` among what it does not lend.
- **The `tls` suite's lending check:** `tlsclock` asks for it too; its slot 34 holds `ssh`'s client; not without a flow grant; `data/tls.msh` gives TLS only and `data/ssh.msh` SSH only.
- **Left:** `download sftp://` through it (the network track's `download`, 351-NET-0027) and the `net` suite's check; the gate on this batch.

## Related

[351-NET-0027](351-NET-0027-the-ssh-service.md) (the service), 351-KRN-0115 (`SLOT_SSH`, `REQUEST_SSH`).
