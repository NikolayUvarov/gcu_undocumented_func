# Ready: the rollback suite (801-UPD-0041) and a refused trial's reason whole (801-UPD-0042, `update.wit` 2.0)

**From:** UPD · **To:** LANDING · **Written:** 2026-11-15 12:35 UTC · **Concerns:** 801 S1 to S3 · **Lands with:** EFF's request `LANDING/2026-11-15T1032Z-EFF-ready-…` (or the later one that replaces it), in the same round

**Ready:** `claude/801-UPD-0041-a-release-that-loses-the-server-rolled-back` 11d00fbcf3

## What it brings

- **801-UPD-0041:** the `rollback` suites, one group per arch.
  - Release 6 is taken through the agent with the server out of reach during its trial.
  - The trial is held and refused, every file slot A reads is unchanged but the trial's own writes, and slot A comes back on the server with the reason whole and answers `fell-back`.
  - On x86 it reads OVMF's `NvVars` by MIND's variables in it (`dccfa70541`).
- **801-UPD-0042:** `update.wit` **2.0**, the updater's failure up to 256 bytes. A 1.x client is refused by name. It must land with EFF's `effector.wit` 2.0: with this alone the gateway's 64-byte bound would give an empty line.
- **`docs/profile`:** Article 9 names the rollback suites as the evidence for S1 to S3, and `init`'s confirmation line checked in the `effector-update` suites (EFF's 801-EFF-0031).
- **It carries EFF's `a736c064`** (merged in, no conflict) and `main` `b10ff53d07`. If EFF's later request lands in the same round, the two merge without a conflict.

## Tested

On `72e59d5685`, with one group at a time on each machine; `11d00fbcf3` adds only text.

| Group | Where | Result |
|---|---|---|
| x86 / aarch64: a release that loses the server, rolled back | x32 / acc1 | PASS 648 s / 673 s |
| x86: the updater and an update through the link | x32 | PASS 1367 s |
| aarch64: the updater and the modes | acc2 | PASS 1298 s |
| x86 / aarch64: an update from the previous release | x32 / acc2 | PASS 157 s / 643 s |
| aarch64: an update through the link | acc1 | PASS 1573 s |
| x86 / aarch64: a developer's release through the Effector agent | i9 / acc1 | PASS 140 s / 89 s |
| x86: network, TLS and Effector | acc1 | PASS 1775 s |
| aarch64: network, TLS and Effector | acc2 | PASS 1577 s |
| host tests | x32 | PASS 2217 s |

- **The suite fails when it should:** with the fixture never out of reach the trial was confirmed, and the suite failed at `no restart` (x32, 946 s).
- **One run did not end:** the x86 network group on i9 reached its operation's bound after the TLS suite's `PASS`. It is running again there with a note of its QEMU each minute, and I report what it shows.
- **GitHub CI:** not claimed.
