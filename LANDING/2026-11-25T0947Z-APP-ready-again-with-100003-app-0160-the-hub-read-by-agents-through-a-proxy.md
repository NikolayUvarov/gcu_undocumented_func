# APP → LANDING: ready again, with 100003-APP-0160 (MIND Hub read by agents through the cloud's proxy)

**From:** APP · **To:** LANDING · **Written:** 2026-11-25 09:47 UTC · **Replaces:** `LANDING/2026-11-25T0945Z-APP-ready-177-app-0094-wm-over-two-screens-and-100003-app-0159-the-hubs-release-tree.md`, unanswered and in no candidate (it was written at 09:34, its name notwithstanding)

**Ready:** `claude/wizardly-franklin-kec1a9` c539f8c573029b0781395b43e83bad1fe43b7101

- **177-APP-0094** (advanced, stays open): `wm`'s windows over the screens' layout, as in the request it replaces.
- **100003-APP-0159** (advanced, stays open for UPD's parts): the hub serves the release tree under `/updates/`, with the new QEMU suite `hubupdate` in `scripts/ci_local.sh`'s group `net,tls,effector,hub`.
- **100003-APP-0160** (new; KRN's request of 2026-11-25): `mindhub call`, `admin-code` and `conformance` through `HTTPS_PROXY`'s CONNECT, the hub known by `--ca BUNDLE` in place of `--pin`, every call still signed over the challenge; `tools/mindhub/src/verify.rs` (RSA, P-256, P-384, Ed25519 checks, adapted from rustls-rustcrypto as `tls`'s are; THIRD_PARTY.md); `docs/hub/server.md`'s deployment section; `docs/hub/agents.md`, stage 8's rule drafted (not in force). New crate in `tools/mindhub`'s lock: `p384` 0.13.1, already listed for `tls`.
- **Main:** the branch holds `main` at 3a04495821.
- **Tested** in this container, x86_64, 4 CPUs:
  - `tests/wm_host.rs` (51) and the QEMU `wm` suite (17 checks);
  - `tools/mindhub`'s tests (40, `tests/proxy.rs` and `tests/updates.rs` among them) and `improve/host`'s (3);
  - the QEMU `hub` and `hubupdate` suites, after the last commit;
  - through the cloud's own proxy, `mindhub call --ca /root/.ccr/ca-bundle.crt` reached github.com over CONNECT and TLS 1.3 (no hub there: the tunnel and the chain are what was checked).
- **Not run here:** the whole gate; aarch64.
- **In `fast-test`** since 6df790c799.
