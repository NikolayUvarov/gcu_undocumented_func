# 700-BLT-0010 — The tool `bt`: scan, pair, list, forget, connect

**Type:** tool (`bt`), interface (`idl/bluetooth.wit`) · **Owner:** `BLT` · **Priority:** P2 · **Status:** open (2026-11-26) · **Blocked by:** `KRN`: `bt_hci`'s service endpoint and the shell's client of it · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-3.1, MC-12.4

## Problem

For the testing stage the devices are named in `data/bluetooth/devices.txt` (700-BLT-0005). The addresses are read from the listening rounds' log. A person at the machine needs a tool instead.

## Plan

1. **`idl/bluetooth.wit` 1.0:**
   - `scan`: the devices heard, with name, class and signal;
   - `pair` and `connect` for an address and a kind;
   - `list`: the devices and their state;
   - `forget`: the key dropped.

   It goes into `idl_fuzz_host`'s list.
2. **`bt_hci` serves it.** The devices it adds go into the list file.
3. **The tool `bt`** in the shell.

## Acceptance criteria

- [ ] Host tests of the interface's types (idl_fuzz_host).
- [ ] In QEMU: `bt list` answers with no controller.
- [ ] On the Ryzen: `bt scan`, then `bt pair` of the maintainer's device.

## Related

[700-BLT-0005](700-BLT-0005-connections-carry-data.md).
