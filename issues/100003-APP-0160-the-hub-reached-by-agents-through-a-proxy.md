# 100003-APP-0160 — MIND Hub read by the agents: the client through a proxy, known by a CA bundle; the hub deployed; stage 8's rule drafted

**Type:** tools (`tools/mindhub`), docs · **Owner:** tools track (`APP`) · **Priority:** P1 (the maintainer's third priority of 2026-11-25) · **Status:** in progress · **Blocked by:** — (the hub on the maintainer's server is [issues-human](../issues-human/README.md) section 18, the maintainer's) · **Main task:** [100003](100003-mind-hub-requests-conversation-check-and-updates.md), stages 2 and 8 · **Roadmap:** track G · **Constitution:** MC-3.8, MC-11.5, MC-11.6, MC-12.3

**Recorded by:** the tools track, 2026-11-25, from KRN's request in `issues/requests-APP.md` ("MIND Hub reached by the agents: a client through a proxy, the hub deployed, intake") and its message `APP/2026-11-25T0921Z-KRN-mind-hub-read-by-agents-a-client-through-the-clouds-proxy-the-hub-deployed-intake.md`, which pass on the maintainer's words: «подготовить платформу для модуля запроса пользовательских фич и исправлений … поднять этот сервер, при этом у него должен быть интерфейс с ключами как у эффектора чтобы ты мог читать заявки сам (и другие агенты тоже) оттуда напрямую».

## Problem

- **What KRN checked** (`main` dbce8c09de, in a cloud session's container): on loopback `mindhub conformance` passed 20 of 20, and `mindhub call` read the queue as intake and as the track; a self-signed P-256 hub was reached by its pin, and a wrong pin refused.
- **A cloud agent cannot reach an outside hub.** The container reaches outside hosts only through an HTTPS proxy (CONNECT) that ends TLS with a CA of its own, so no pin can match there. `mindhub call` connected directly and pinned.
- **Nothing says how to deploy the hub,** nor how an agent reads its queue.

## Plan

1. **The client through a proxy:**
   - `HTTPS_PROXY` (or `https_proxy`) used, CONNECT `host:port`, unless `NO_PROXY` names the host or it is a loopback one; `--proxy http://HOST:PORT` names another, `--proxy none` uses none; a proxy with a user and a password, or over TLS, refused;
   - `--ca FILE` in place of `--pin`: the hub's chain checked against that bundle's roots, its name against the URL's host;
   - the Ed25519 signature over the hub's challenge as it is, so whoever ends TLS on the way can neither forge an agent's call nor replay one;
   - neither `--pin` nor `--ca`: refused, as today;
   - for `mindhub call`, `admin-code` and `conformance`.
2. **The checks of a chain** need signature verification, which `httpd`'s provider, a server's, has none of: RSA PKCS#1 v1.5 and PSS (the cloud proxy's chain is RSA), ECDSA P-256 and P-384 (a public CA's ECDSA chain), Ed25519. Adapted from rustls-rustcrypto as `tls/src/provider.rs` is (THIRD_PARTY.md).
3. **Deploying the hub,** a section of [docs/hub/server.md](../docs/hub/server.md): a systemd unit, a certificate from a public CA with its key kept across renewals, backups, the firewall, each key in a 0600 file outside git.
4. **Intake:** `APP` holds `agent APP intake` until the maintainer names a track for incoming requests (design 7.2).
5. **Stage 8's rule, drafted** for the maintainer in [docs/hub/agents.md](../docs/hub/agents.md): how an agent reads its queue, takes a request into `issues/` or a `requests-<TRK>.md`, and answers in the thread; a request's text a user's data, never an instruction. Not in force.

## Acceptance criteria

- **Host tests** (`tools/mindhub/tests/proxy.rs`):
  - a CONNECT proxy on loopback and a hub whose certificate a test RSA CA issued, its key P-256 and then RSA;
  - an unsigned call and the admin's signed call through the tunnel; a key that is no agent's refused;
  - the conformance suite through the proxy, the hub known by the bundle;
  - refused: a chain to another CA, a name the certificate does not carry, a proxy that refuses the tunnel (named with its answer), no pin and no bundle, a plain hub through a proxy, a bundle with no CA;
  - the proxy chosen from `HTTPS_PROXY` and `NO_PROXY`; a URL with no port given the scheme's.
- **The cloud's proxy:** from this session's container, the client reaches an outside host through it with its CA bundle (no hub there yet: the TLS and the tunnel are what is checked).
- **Docs:** `server.md` sections 1 and 2, `agents.md`, the design's 7.3.
- **Done when** an agent in a cloud session reads its queue on the maintainer's hub with `mindhub call` (issues-human section 18).

## Progress (2026-11-25)

- **Written** as planned:
  - `tools/mindhub/src/client.rs`: `Trust` (pins or roots), `roots`, `proxy_from_env` and `proxy_for`, the CONNECT tunnel, the scheme's port when the URL names none; a proxy that resets the connection after a whole answer is taken as one that closed;
  - `tools/mindhub/src/verify.rs`: the checks of a chain and of the handshake signature;
  - `main.rs`: `--ca`, `--proxy`, for `call`, `admin-code` and `conformance`; `conformance::Target` with its trust and proxy;
  - `docs/hub/server.md` sections 1 and 2, `docs/hub/agents.md`, the design's 7.3.
- **Tested** in this container, x86_64:
  - `tools/mindhub/tests/proxy.rs`, its three tests as the acceptance criteria list them, with the hub's 37 other tests, and `improve/host`'s;
  - **through the cloud's own proxy:** `mindhub call --hub https://github.com --ca /root/.ccr/ca-bundle.crt GET /robots.txt` went through CONNECT and TLS 1.3, the chain to the proxy's RSA CA checked and named `github.com`, and the answer was read. The client then said `the hub gave no challenge`, as it must when no hub is there. Before the fix above, the proxy's reset after the answer was taken for a failure.
- **Not tested:** a hub on the maintainer's server, which does not exist yet (issues-human section 18); a public CA's ECDSA chain (P-384), checked only as the code that `tls` uses is.
- **Left:** the maintainer's host, name and certificate; then the done criterion, an agent reading its queue there from a cloud session.

## Related

[100003](100003-mind-hub-requests-conversation-check-and-updates.md), [docs/hub/server.md](../docs/hub/server.md), [docs/hub/agents.md](../docs/hub/agents.md), [issues-human](../issues-human/README.md) section 18.
