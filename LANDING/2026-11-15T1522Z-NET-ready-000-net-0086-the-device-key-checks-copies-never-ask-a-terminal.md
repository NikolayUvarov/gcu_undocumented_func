# Ready: the device key check's copies never ask on a terminal (000-NET-0086)

**From:** NET · **To:** LANDING · **Written:** 2026-11-15 15:22 UTC · **Concerns:** the gate's lanes, i9

**Ready:** `claude/000-NET-0086-mtools-never-asks-a-terminal` 479fbef4aa

## What it brings

**`tests/qemu_smoke.py` only:** the four copies from the disk image to a local file in the device key check pass `mcopy -n`.

**Why:**
- On i9 the agent's operations are background process groups of a terminal's session.
- mtools 4.0.43's `mcopy`, even with `-o`, asks on the terminal before it overwrites a local file.
- That read stops the whole group (SIGTTIN). Three runs of the x86 network group stood still on i9 until their bound.
- Without a terminal, as on every other machine, nothing changes.

The issue's text also records it.

## Tested

| Group | Where | Result |
|---|---|---|
| x86: network, TLS and Effector | i9 (on `0bec0bef31`) | PASS 2081 s; it had stopped three times without the fix |
| x86: network, TLS and Effector | acc1 (on `77118bf7bf`, with round 141's `main`) | PASS 1809 s |
| aarch64: network, TLS and Effector | acc2 (on `77118bf7bf`) | PASS 1573 s |

`479fbef4aa` adds only text.
