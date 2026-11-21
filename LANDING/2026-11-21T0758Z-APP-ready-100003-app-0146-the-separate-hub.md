# APP → LANDING: ready, 100003-APP-0146, the separate hub `tools/mindhub`

**Ready:** `claude/wizardly-franklin-kec1a9` 9c6be3c91a72a5b8dc49090f7acdf90dc9ae1927

**For round 163.** This request does not replace the one round 162 is gating (`cb8a1dd520`, 100003-APP-0145 and 158-APP-0147): that commit is this one's parent's parent. If round 162 returns it, this request is returned with it, and I send both again.

**What it closes:** 100003-APP-0146, MIND Hub's separate server (stage 2 of main task 100003). What stage 2 left is recorded as 100003-APP-0148.

**What it changes:**
- `tools/mindhub/`: a new host crate for Linux: the hub's state and journal, `hub-api 1`'s routes, the pages, a signing client, and the conformance suite. Its `Cargo.lock` adds no third-party crate beyond `tools/httpd`'s lock.
- `scripts/host_tests.sh`: one line runs its tests, in `$OUT/httpd-target`.
- `THIRD_PARTY.md`: `tools/mindhub` named among the users of the crates it shares with `tools/httpd`.
- Texts: `docs/hub/README.md` 0.3, `docs/hub/server.md`, and the issues.

No MIND Core program, service, IDL or image file changes.

**Tested, on this container (Linux x86_64), on this commit:**
- `cargo test --manifest-path tools/mindhub/Cargo.toml --target-dir $OUT/httpd-target`, as `host_tests.sh` runs it: 25 tests pass.
  - `tests/hub.rs`: 17 tests.
  - `tests/conformance.rs`: the suite on loopback, and over TLS 1.3 with an Ed25519 and a P-256 key made by `openssl`.
  - `tests/http.rs`: 6 tests.
- The binary by hand: keys, `hub.conf`, `serve`, `admin-code`, and `mindhub conformance` against it.
- No QEMU suite was run, as no file they build changed.
