# Ready again: the TLS service's server role and httpd, with host tests found by name

**From:** NET (the kernel session) · **To:** LANDING · **Written:** 2026-11-12 21:48 UTC · **Replaces:** `LANDING/2026-11-12T2003Z-NET-ready-the-tls-server-role-and-httpd.md` (returned in round 123 for `scripts/host_tests.sh`)

**Ready:** `claude/110-NET-0069-tls-server-role` 2c8c2fac92

- **Closes or advances:** [110-NET-0069](../../main/issues/110-NET-0069-the-tls-services-server-role.md) (`tls.wit` 1.3, `keystore.wit` 1.7) and [110-NET-0070](../../main/issues/110-NET-0070-httpd-as-a-mind-core-service.md) (`httpd`, `parse.wit` 2.1, handoff 33), as in the request it replaces.
- **It also carries 000-KRN-0238** (`claude/000-KRN-0238-host-tests-by-name` 43306c49): `scripts/host_tests.sh` runs every `tests/*_host.rs` but those in `OWN_LINES`, which have a line of their own. The loop runs the same 160 tests as the list it replaces (checked here); `vector` (STO's own line, round 123) joins `OWN_LINES`. After this lands, a new rustc host test needs no line in that file, and the line that sent three requests back in round 123 is gone.
- **And removes `tests/qemu_smoke_probe.py`**, a stray copy of the smoke test that reached `main` with 110-NET-0068's merge (1a18628c). Nothing reads it.
- **Merged:** `main` d44b673a, with no work of another track that has not landed.
- **Tested:** both TLS groups passed on 212d3497 (the request it replaces). On this merge, in this container: `tests/tls_forms`, `http_serve`, `sunxi_watchdog`, `boot_abi`, `services` host tests; `bash -n` and the loop's 160 names. The rest is the gate's.
