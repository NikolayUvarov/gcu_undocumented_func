# KRN ready: a program's secrets (ABI 24, keystore.wit 1.4), the browsing grant (socket.wit 2.7) and `get`

**Ready:** `claude/youthful-mendel-mf1soy` 873f18c8

The request `LANDING/2026-10-28T1354Z-KRN-ready-secrets-browsing-get-and-the-new-lanes.md` (cb7f8f14) was returned in round 40: a conflict in `scripts/host_tests.sh` with STO's request. `main` 8016f87f is merged in and the conflict resolved: STO's list (`blockstore` run on its own line) with `secrets` and `get` added. This is for round 41. It replaces `LANDING/2026-10-29T0745Z-KRN-ready-again-after-round-40.md` (48802a43), whose gate the integrator stopped at once: its own `ci_local.sh` check for Python 3.11 stopped "build (x86)" on `acc-1` and `acc-2`. 873f18c8 limits the check to the host tests group.

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
- **The landing queue.**
  - 287ca3ef: `scripts/land.py` counts a closed issue (`issues-done/*.done`) as text, as AGENTS.md section 4 says; round 39's push had refused `main`'s closing commit as code. `tests/land_test.py` checks it.
  - 819382d0, ca21c387: the maintainer's new gate machines `acc-1` and `acc-2` as lanes (`acc1`, `acc2`), the preset `i9-on-rz` of rounds 34 to 39, and the presets `wide` and `wide-no-i9` (docs/effector.md, sections 1 and 4, already on `main` 4ecde4a8).
  - 0954d35e: a lane with the host part builds the x86 image first, as the host tests read its ELF files.
  - 7b14ea95, 873f18c8: `scripts/ci_local.sh` names a missing Python 3.11 (`tomllib`) up front when the host tests group runs.

## Tested (this container, QEMU, 4 CPUs)

- On 48802a43 (after the merge): `02_build.sh` and the whole `scripts/host_tests.sh` (157 test programs, all passed).
- Before the merge, on the branch's own commits:

- Host tests: `secrets`, `netpolicy`, `get`, `tests/land_test.py`, `runtime`, `idl_fuzz`, `services`, `shell`, `boot_abi`, `tests/idl_test.py`.
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
- `fast-test` carries all of them (merged next).
- Round 40 is gated with the preset `wide` (or `wide-no-i9` if i9 is busy): the long x86 groups on `acc-1` and `acc-2`.
