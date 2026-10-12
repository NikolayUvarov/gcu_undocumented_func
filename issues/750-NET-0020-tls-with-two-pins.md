# 750-NET-0020 — A server known by one of two pins, its certificate still checked

**Type:** network (service, interface) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) (with 501) · **Roadmap:** track D · **Constitution:** MC-11.5, MC-12.4

The pin part of the request in requests-NET.md ("HTTP for a service, a certificate pin and long-lived sessions").

## Problem

Effector's agents pin the server by the SHA-256 of its leaf certificate, with a second pin for rotation, and still check the certificate's name. `idl/tls.wit` 1.1 `connect-pinned` takes one SPKI pin and checks no name, root or validity period.

## Plan

- **`idl/tls.wit` 1.2 `connect-pins`:** one or two pins, each of the leaf (its DER) or of its SPKI. A server matching neither is refused (`certificate`).
- **Still checked with a pin:**
  - the name against `name` (webpki);
  - the validity period by the system clock;
  - the server-authentication purpose when the certificate names purposes;
  - `CA:false`;
  - the handshake's signature by the certificate's key.

  The issuer is not looked for: the pin is what is trusted, and a server may send its leaf alone. The fields are read with `x509-cert` (THIRD_PARTY.md).
- **`connect-pinned` (1.1)** keeps its meaning.
- **TLS 1.3 only,** as now.

## Acceptance criteria

In QEMU (the `net` suite's download check, x86 and aarch64), with `webcheck --pin`:
- **Accepted:** a server trusted by its leaf pin; by its SPKI pin as the second of two (rotation); by a pin with its DNS name.
- **Refused:**
  - a server matching neither pin;
  - a wrong name (`NotValidForName`);
  - an expired certificate with its own leaf pin (`Expired`).

  Each refusal is logged by `tls`.

## Related

[351-NET-0002](../issues-done/351-NET-0002-https-for-programs.done), [docs/network/downloads.md](../docs/network/downloads.md).
