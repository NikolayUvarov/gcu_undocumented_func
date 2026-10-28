# KRN ready: a program's secrets (ABI 24, keystore.wit 1.4), the browsing grant (socket.wit 2.7) and `get`

**Ready:** `claude/youthful-mendel-mf1soy` 3f4fa0aa

This replaces the unanswered `LANDING/2026-10-28T0853Z-KRN-ready-secrets-and-the-browsing-grant.md` (b4759726). It is for the round after round 39 and adds one commit on top of b4759726.

## What it lands

- **83b4ac12: [000-KRN-0182](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/youthful-mendel-mf1soy/issues/000-KRN-0182-a-programs-secrets-client.md) and [000-NET-0045](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/youthful-mendel-mf1soy/issues/000-NET-0045-secrets-per-program-in-keystore.md) (advance).**
  - `SLOT_SECRETS` 40, `SLOT_DYNAMIC` 41, `CAP_SLOTS` 105, **ABI 24**.
  - The loader lends a program asking for `REQUEST_SECRETS` its client of its own secrets.
  - `keystore.wit` **1.4**: one sealed file per program.
- **b4759726: [000-NET-0049](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/youthful-mendel-mf1soy/issues/000-NET-0049-a-flow-grant-asked-at-run-time.md) (advances).**
  - `socket.wit` **2.7**: a rule of address 0 reaches any public address over TCP on its port.
  - `netpolicy` reads `browse <program> public tcp <ports> …`.
  - `netstack` logs each new host such a grant reaches.
- **3f4fa0aa: [000-NET-0048](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/youthful-mendel-mf1soy/issues/000-NET-0048-get-a-wget-and-curl-analog.md) (advances).**
  - A new program, `get`: the console's `wget` and `curl`, with curl's exit statuses.
  - `tests/http_api_stub.py` gains an echo, two redirects and a slow answer; the `net` suite's download check runs `get_runs`.

## Tested (this container, QEMU, 4 CPUs)

- Host tests: `secrets`, `netpolicy`, `get`, `runtime`, `idl_fuzz`, `services`, `shell`, `boot_abi`, `tests/idl_test.py`.
- x86 suites:
  - `tls`, with the secrets check across a reboot;
  - `net`, whole, with the browsing check and `get_runs`;
  - `boot`, `services` and `bench`.
- aarch64 suites:
  - `tls`, with the secrets check;
  - the `net` suite's browsing check and `get_runs`.

## For the other tracks

- The ABI moves to 24, so every program is rebuilt with it.
- `bt_hci` (700-KRN-0176) comes back with ABI 25 and handoff 25.
- The `net` group runs about 3 minutes longer with `get_runs` (x86, TCG).
- `fast-test` 53444ecb carries all three commits.
