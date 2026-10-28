# KRN ready: a program's secrets (ABI 24, keystore.wit 1.4) and the browsing grant (socket.wit 2.7)

**Ready:** `claude/youthful-mendel-mf1soy` b4759726

This replaces the unanswered `2026-10-28T0819Z-KRN-ready-secrets-client-and-keystore-1-4.md` (83b4ac12). It is for the round after round 39 and adds one commit on top of 83b4ac12.

## What it lands

- **83b4ac12: [000-KRN-0182](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/youthful-mendel-mf1soy/issues/000-KRN-0182-a-programs-secrets-client.md) and [000-NET-0045](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/youthful-mendel-mf1soy/issues/000-NET-0045-secrets-per-program-in-keystore.md) (advance).**
  - `SLOT_SECRETS` 40, `SLOT_DYNAMIC` 41, `CAP_SLOTS` 105, **ABI 24**.
  - The loader lends a program asking for `REQUEST_SECRETS` its client of its own secrets.
  - `keystore.wit` **1.4**: one sealed file per program.
- **b4759726: [000-NET-0049](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/youthful-mendel-mf1soy/issues/000-NET-0049-a-flow-grant-asked-at-run-time.md) (advances).**
  - `socket.wit` **2.7**: a rule of address 0 reaches any public address over TCP on its port.
  - `netpolicy` reads `browse <program> public tcp <ports> …`.
  - `netstack` logs each new host such a grant reaches.

## Tested (this container, QEMU, 4 CPUs)

- Host tests: `secrets`, `netpolicy`, `runtime`, `idl_fuzz`, `services`, `shell`, `boot_abi`, `tests/idl_test.py`.
- x86 suites:
  - `tls`, with the secrets check across a reboot;
  - `net`, with the browsing check;
  - `boot`, `services` and `bench`.
- aarch64 suites:
  - `tls`, with the secrets check;
  - the browsing check of the `net` suite.

## For the other tracks

- The ABI moves to 24, so every program is rebuilt with it.
- `bt_hci` (700-KRN-0176) comes back with ABI 25 and handoff 25.
- `fast-test` d44633d8 carries both commits.
