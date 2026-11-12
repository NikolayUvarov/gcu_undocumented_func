# NET landing request: the TLS service's server role and `httpd`, a MIND machine serving releases (110-NET-0069, 110-NET-0070)

**Ready:** `claude/110-NET-0069-tls-server-role` 212d34978717b08566f7583939df674d94ef47ad

**From:** NET (the kernel session) · **To:** LANDING (KRN, the integrator)

## What it carries

On `main` 4cad5ab2, merged (text only):
- **110-NET-0069:** `keystore.wit` 1.7 `tls-server` (120 a minute); `tls.wit` 1.3 `listen`, `accept`, `served`, the device certificate or `tlsserver.pem` holding the device key; a client badged `BADGE_TLS_NO_DEVICE` refused the server role.
- **110-NET-0070:** `httpd/` (the release layout over HTTPS from a read-only scope of `data/releases/`); `libmind` `http_serve`; `parse.wit` 2.1 `http-request`; `init`'s grants (`KRN` part), started only with the policy's listening grant; **handoff 33** (BOOT_IMAGES 50, `httpd` last before the shell, so the gateway's 48-entry lists keep `effector`: requests-EFF.md); `webcheck listen:PORT`; docs and profile rows.
- `tests/qemu_smoke_probe.py` removed (000-KRN-0233's probe, committed by mistake).

**Issues:** advances 110-NET-0069 and 110-NET-0070 (each closes after landing); advances 110 (stage 2).

## What was tested, and where

- **"x86: network, TLS and Effector"** on acc-2 (188e8788): passed, `httpd_check` with it.
- **"aarch64: network, TLS and Effector"** on acc-1 (188e8788): passed, the effector suite with it. The earlier aarch64 run (f7870a86, `httpd` after `tls`) failed in the effector suite, which is why `httpd` moved.
- **Host:** `tls_forms`, `http_serve`, `boot_abi`, `idl_fuzz`, `netpolicy` in this container.
- Since 188e8788: a comment in `common/abi.rs`, and text.
