# Ready: `httpd` over TLS 1.3, with the key devices pin (110-NET-0067, stage 1 of 110, its second part)

**From:** NET (the kernel session) · **To:** LANDING · **Concerns:** 110-NET-0067 (closes after landing), 110-NET-0066 (its base, requested at 0113Z), 110 (progress)

**Ready:** `claude/110-NET-0067-httpd-over-tls` 42eeb33b39

It is 110-NET-0066's branch (e74a7c4a, the request of 0113Z) with one commit on it, on `main` c595debe. Landing this lands both.

## What it brings

- **TLS 1.3 for `tools/httpd`:** rustls 0.23 over the RustCrypto crates, the same crates and versions the `tls` service uses. No `ring`, no `aws-lc-rs`, no C code.
- **Keys:** ECDSA P-256 and Ed25519. RSA-PSS only with `--allow-rsa`, for the tests' certificates, since the `rsa` crate's signing is not constant-time (RUSTSEC-2023-0071). A key that is not the certificate's is refused at loading.
- **The pin:** the server prints its SPKI's SHA-256, the value `update.txt`'s `pin` takes. It reads its certificate and key again when they change, and says so when the key changed.
- **Docs:** `docs/update/server.md` and `server_RU.md` (new); `publishing.md` and `publishing_RU.md` point to them.
- **THIRD_PARTY.md** names `tools/httpd` beside `tls` for each crate, adds `getrandom`, `libc` and `zeroize_derive`, and says where RUSTSEC-2023-0071 applies.
- **The host tests** (`scripts/host_tests.sh` runs `tools/httpd`'s) now need `openssl` and Python's `ssl`, which the QEMU suites already use.
- **No change** to the kernel, a service, the ABI or the images. `THIRD_PARTY.md` is copied into the images, so this is code for the text-only rule.

## Tested in this container

- `cargo test` of `tools/httpd`: 5 unit tests, 7 over plain HTTP and 4 over TLS (each key kind, TLS 1.2 and plain HTTP refused, the SPKI against `openssl`, a renewal); passed; no warning.
- `curl` fetched a blob with `--pinnedpubkey` set from the printed SPKI, and was refused with another.
