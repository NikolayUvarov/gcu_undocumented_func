# 111-NET-0073 — The TLS service offers an application protocol and derives keying material: `tls.wit` 1.4 for NTS-KE

**Type:** network (`tls`, `idl/tls.wit`) · **Owner:** `NET` · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [111](111-network-time.md) (stage 2) · **Roadmap:** track D · **Constitution:** MC-5.6, MC-11.9, MC-11.11, MC-12.2, MC-12.4

## Problem

NTS-KE (RFC 8915, section 4) runs over TLS 1.3. The client offers the application protocol `ntske/1` (ALPN, RFC 7301), and the keys that seal the NTP packets come from the session's exporter (RFC 8446, section 7.5) with the label `EXPORTER-network-time-security`. `tls.wit` 1.3 offers neither, so `timed` cannot run NTS-KE through `tls`. It must not hold the TLS keys itself (MC-11.9, 111's plan).

rustls's unbuffered connections, which `tls` uses, have no public exporter: only its buffered connections do.

## Plan

1. **`tls.wit` 1.4** (MC-12.4: a new version, the earlier calls unchanged):
   - `connect-alpn(session, name, address, port, protocol, timeout-ms)`: as `connect` without the device certificate, offering one application protocol (up to 32 bytes). A server that does not agree on it is refused (`handshake`), and `tls` logs why.
   - `export(session, label, context, length)`: 1 to 64 bytes of the established session's exporter, for the session's own client. `invalid` before the handshake is done; `again` while a served handshake goes on.
2. **The exporter in `tls`** (`tls/src/exporter.rs`):
   - each connection, client or served, gets a key log of its own that keeps the exporter secret alone;
   - `tls` derives the keying material from it by RFC 8446's HKDF-Expand-Label, with the suite's hash;
   - every copy it used is wiped. No other secret is logged, and none leaves `tls`.
3. **Not here:** ALPN on the server's side, and more than one protocol offered. NTS-KE needs neither.

## Acceptance criteria

- **Host:** `tests/tls_exporter_host.rs` derives what OpenSSL derives, for a SHA-256 and a SHA-384 suite, from the exporter secret OpenSSL's key log holds. Lengths of 0 and 65, and a label of 250 bytes, are refused. The key log keeps the exporter secret and nothing else.
- **QEMU, x86 and aarch64:** `timed`'s NTS-KE through `tls` agrees on `ntske/1` and derives keys the NTS server shares ([111-NET-0075](111-NET-0075-timed-with-nts.md)). A server that agrees on no protocol is refused.
- The service builds for both targets; the IDL test passes.

## Progress

**Made (2026-11-12)** on `claude/111-NET-0073-tls-alpn-exporter`, on 110-NET-0069's branch (`tls.wit` 1.3):
- `idl/tls.wit` 1.4 and its bindings; `tls`'s `connect-alpn` and `export`; the per-connection key log (`Keys`) and `exporter.rs`.
- **Tested** (this container): `tests/tls_exporter_host.rs`, 4 tests, against two vectors made with OpenSSL 3.0.13 (`s_client -keylogfile`, `-keymatexport EXPORTER-network-time-security -keymatexportlen 32`, both ends printing the same bytes). They pass. `scripts/host_tests.sh` runs them. `tls` builds for `x86_64-unknown-none` and `aarch64-unknown-none-softfloat`; `tests/idl_test.py` passes.
- **QEMU, x86** (this container, 111-NET-0075's `nts_check`): `ntske/1` agreed with Python's `ssl` (OpenSSL) and with `chronyd` 4.5 (GnuTLS); keys from `export` that both servers derived too, under SHA-384 (OpenSSL chose AES-256-GCM) and SHA-256 (GnuTLS chose ChaCha20-Poly1305); a server agreeing on no protocol refused, and `tls` logged why.
- **Left:** aarch64, on the maintainer's machines; the gate.

## Related

- [111](111-network-time.md): the main task, stage 2.
- [110-NET-0069](110-NET-0069-the-tls-services-server-role.md): `tls.wit` 1.3, the server role.
- [111-NET-0074](111-NET-0074-a-grants-host-names.md), [111-NET-0075](111-NET-0075-timed-with-nts.md): the rest of stage 2.
