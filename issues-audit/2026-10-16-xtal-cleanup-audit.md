# H14 — XTAL cleanup can replace an unfinished board-access command

**Assessment:** corrected by `37a379a4`; host regressions passed after raw publication.
**Priority:** P1 · **Owner:** ASTRA, [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md)
· **Roadmap:** H / 213, D / 550 · **Constitution:** MC-6.1, MC-12.1, MC-12.3.

## Finding

`power::xtal_write` and `power::xtal_read` wrote a new indirect command before
checking whether the previous command was still busy. When board power-on or
DAV access timed out, `board::read` correctly gave shutdown a fresh budget,
but `power::off` could overwrite that same unfinished XTAL command. The older
fixture even expected shutdown success after the model's first command stayed
busy indefinitely; the replacement made the model complete a different command.

The later native MAC/RX cleanup already had a preflight idle-XTAL boundary,
but the earlier board-access cleanup and shared helpers did not. This finding
is independent of the unimplemented warm-device reset transition; it does not
justify weakening the already-active-MAC guard or testing on a live Linux NIC.

## Correction and acceptance

Both shared XTAL helpers first wait at most 50 ms for the existing command to
finish, within the caller's tighter stage budget. Unavailable registers, stuck
or regressing clocks and expired budgets fail before a new command write.
A completed previous command permits the next one normally. A busy-board
cleanup preserves both the primary error and its own timeout; it cannot claim
power-off or overwrite the unfinished command.

Focused regression cases exercise both read/write entrypoints, unavailable
reads, a frozen clock, successful delayed completion, a tighter nested budget
and the complete board-failure/independent-cleanup path. Existing successful
board/OTP cycles and cumulative firmware/MAC/RX scenarios must still pass.
Build on ASTRA, publish to fast-test, then execute the fixtures on ASTRA.
Hardware operation and the overall gate remain pending.

## Execution evidence

Source `37a379a4`, consolidated ASTRA `3226b5fe`, was built on the own branch
and published in `fast-test` `1a62e46f` before execution. On x86-64 WSL2,
`rtw89_power_host` passed all 18 cases (0.10 s); the cumulative MAC/RX harness
passed 84 cases (0.87 s), with no failures and one existing private-file case
ignored. The latter count includes imported module fixtures, not 84 independent
end-to-end scenarios. The private firmware case has separate explicitly
executed evidence in the cumulative document. No physical XTAL or warm-device
behavior is established by these models; the overall gate remains open.
