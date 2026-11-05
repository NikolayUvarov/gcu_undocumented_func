# Ready: the station's reading client, `wifi.wit` 1.3 `reader` (550-NET-0058)

**From:** NET (worked by the storage session) · **To:** LANDING · **Written:** 2026-11-05 19:03 UTC · **Concerns:** 550-NET-0058 (closes), 550-KRN-0213 (its second criterion)

**Ready:** `claude/relaxed-meitner-5bmhpz` fe4732eee4f10e3c4d823bc06f291aa7722e8c04

It contains `claude/550-KRN-0213-wlan-reading-client` at 40624d06, with `main` 6253e7a7, so it lands with or after `2026-11-05T1756Z-KRN-ready-the-stations-endpoint-may-badge-its-reading-client.md`. Without that grant `reader` answers `unsupported` and its new `net` check fails.

## What it brings

- **`idl/wifi.wit` 1.3** (minor: a function added at the end), with its generated `libmind/src/idl/wifi.rs`. `reader` answers a client of the station that only reads (`BADGE_WIFI`), or `unsupported`.
- **`wlan`:** mints that client once at its start from its own endpoint, never `BADGE_WIFI_CHANGE` and never an unbadged client, and answers it to any client of the interface. Without the badge right it logs `[WLAN] NO READING CLIENT`.
- **`netcheck wifi`:** takes the reading client through the client it was lent and drops that one. It reads the link and the country, and is denied `join`, `leave`, `forget` and `set-country`, with the station as it was.
- **`net` suite:** `wificheck wifi` in the joined-station check (a copy of `netcheck.elf` with `REQUEST_WIFI`). Its `wifi country` check reads only `… APPLIED`, so APP's wording stays APP's.
- **Docs:** `docs/network/station.md` 0.2 ("Who may change the station"), the `wifi` row of `docs/idl/README.md`, and the `wlan` row of `docs/profile/bootstrap.md`, which has KRN's sentence folded into NET's.
- No change to the ABI. `init` changes only through 550-KRN-0213.

## A text conflict to expect

`000-NET-0050`'s request changes `bootstrap.md`'s `netstack` row, the line after the `wlan` row. Keep both: this branch's `wlan` row and 0050's `netstack` row. In `fast-test` (20e1c6db) it is resolved that way.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG), on fe4732ee

- **Built** x86 with `scripts/x86_fixtures.sh`, and aarch64 with `--fixtures`, from a clean build.
- **`net` suite:** passed on x86 (35 checks) and on aarch64 (32), `PASS: the station's reading client (550-NET-0058)` among them.
- **`scripts/host_tests.sh`:** every Rust test passed. One Python test failed: `tests/effector_server_test.py`'s `test_contract_tls_pin_auth_and_prefix`.
  - It fails the same way on `main` 6253e7a7 in this container, whose Python is 3.13.18 and OpenSSL 3.5.9.
  - The fixture's server gets `BrokenPipeError` reading the request line of the connection the test opens and closes without sending a request.
  - It is EFF's, and is reported to EFF. This branch touches nothing of Effector.
