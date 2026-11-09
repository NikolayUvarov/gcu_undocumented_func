# 000-KRN-0224 — A request bit for a console program's secret line, and the shell's badge for it

**Type:** `init`, `libmind` (`mind::process`) · **Owner:** kernel session · **Priority:** P2 · **Status:** open (made and tested in QEMU; the lending is APP's) · **Blocked by:** — · **Roadmap:** the tools track · **Constitution:** MC-3.1, MC-3.4

From APP's request of 2026-11-09 for [000-APP-0126](000-APP-0126-a-console-program-reads-a-secret.md), which NET's [000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md) (`get -u user`) waits for (`issues/requests-KRN.md`, and `KRN/2026-11-09T0729Z-APP-a-request-bit-for-a-console-programs-secret-line.md` on `internal-dev-messages`).

## Problem

- **A console program cannot read keys,** and the shell reaches a running program only through what it lent at its start.
- **A text input for every console program** would make `grep`, `sha256` and others, which read their input when they have one, wait instead of printing their usage. A secret on the program's text input would also mix with a pipe's data.
- **So the program says at its start** that it may ask the shell for one secret line, and the shell lends it only that: a copy of its command client with a badge of its own, through which it answers `shell.wit` 1.12 `secret-line` alone (APP's).
- **What the shell lacks for that:** the bit, the badge, and the right to badge on its command endpoint. `init` gives it `SLOT_COMMANDS` with `ALL`, which has no `CAP_BADGE` (000-KRN-0073).

## Plan

1. **`mind::process`:**
   - `REQUEST_SECRET_LINE` = `1 << 29`, the next free bit after `REQUEST_WIFI_READ`;
   - `BADGE_SHELL_SECRET` = 2, beside `BADGE_SHELL_LINK` = 1.
2. **`init`** gives the shell `SLOT_COMMANDS` with `ALL | CAP_BADGE`.
   - The keeper the endpoint is minted from has `CAP_BADGE`, so the shell's receiver keeps it across restarts.
   - A badged copy never carries `CAP_BADGE` (the kernel's `CAP_MINT`), so no program the shell lends a copy to can badge it again.
   - The unbadged copies the shell lends for `REQUEST_SHELL` are minted `CAP_WRITE | CAP_GRANT` (`commands_client`), so they carry no `CAP_BADGE` either.
3. **Docs:** slot 37 in `docs/api/README.md`, the shell's row in `docs/profile/bootstrap.md`.

No ABI change: the request bits and the shell's badges are `libmind`'s, and `CAP_BADGE` exists since ABI 6.

## Acceptance criteria

- The constants are in `mind::process`, and `init` grants the shell's endpoint `ALL | CAP_BADGE`.
- **In QEMU, x86 and aarch64:** the shell starts and answers with it. The `shell` suite (a program's commands client, `link`'s badged client in developer mode) and `normal` pass.
- **APP's part** (000-APP-0126): the shell mints the `BADGE_SHELL_SECRET` copy for `REQUEST_SECRET_LINE`, answers only `secret-line` through it, and its QEMU check shows it.

## Related

[000-APP-0126](000-APP-0126-a-console-program-reads-a-secret.md), [000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md), [550-KRN-0213](../issues-done/550-KRN-0213-the-stations-endpoint-may-badge-its-reading-client.done) (the same shape), 800-KRN-0148 (`SLOT_COMMANDS`), 000-KRN-0073 (`CAP_BADGE`).
