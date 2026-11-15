# 000-NET-0086 — The device key check's copies from the disk never ask on a terminal

**Type:** tests (`tests/qemu_smoke.py`, the device key check of the `tls` suite) · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** open (2026-11-15) · **Blocked by:** — · **Main task:** — · **Roadmap:** the landing gate · **Constitution:** MC-12.2

## Problem

**What happened:** on i9, `x86: network, TLS and Effector` stopped three times on 2026-11-15, each time right after the TLS suite's `PASS`. Run alone, `device_key_check` stopped the same way twice. Each time the operation reached its bound, about an hour later. The same tree passes on acc1.

**What a look at the processes showed** (a second operation, three minutes into a stopped run):
- **Every process of the run was stopped,** in state `T` (`do_signal_stop`): the operation's shell, the test, a `sleep 30`, and `mcopy -o -i …disk.img@@1048576 ::/system/keystore/secrets/netpeer.kept …/read-secrets-netpeer.kept`, which held `/dev/tty` open.
- **The cause:** on i9 the Effector agent runs from an interactive shell on `pts/3`, so each operation is a background process group of that terminal's session. mtools 4.0.43's `mcopy`, even with `-o`, asks `File "…" exists, overwrite (y/n) ?` on the terminal when the local file it copies to exists. A read of the terminal from a background group stops the whole group (SIGTTIN).
- **Elsewhere:** no machine but i9 gives its operations a terminal, so there `mcopy` overwrites without asking.

**Checked here:** with a terminal (`script`), `mcopy -o` onto an existing file asked and was left waiting. With `-n` it overwrote and ended. Without a terminal it overwrote either way.

## Plan

The four copies from the disk image to a local file in the device key check and its helpers (`stored`, `keystore_file`, the store's kept file, a program's kept secret) pass `-n`: no question before overwriting a local file. Nothing else changes, and with no terminal the behaviour is the same as before.

## Acceptance criteria

- `x86: network, TLS and Effector` passes on i9, where it stopped.
- `x86` and `aarch64: network, TLS and Effector` pass on another machine.

## Related

- Round 141's gate, run without i9 for this reason (`ALL/2026-11-15T1257Z-KRN-round-141-gated-on-the-pool-without-i9.md` on `internal-dev-messages`).
- [351-NET-0005](../issues-done/351-NET-0005-persistent-device-key.done): the device key kept on the disk. 308-KRN-0183: the store's kept key.

## Progress (2026-11-15)

**The fix:** `0bec0bef31`, with `main` merged in at `77118bf7bf` (round 141).

**What passed on the maintainer's machines:**
- **`x86: network, TLS and Effector` on i9,** where it had stopped three times: 2081 s, on `0bec0bef31`.
- **The same group on acc1:** 1809 s, on `77118bf7bf`.
- **`aarch64: network, TLS and Effector` on acc2:** 1573 s, on `77118bf7bf`.

**Left:** the gate. Then i9 can take the gate's lanes again.
