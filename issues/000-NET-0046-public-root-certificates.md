# 000-NET-0046 — Public root certificates in the image

**Type:** network (`tls`'s root store, the image builders) · **Owner:** `NET` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — (for 253 to 261 and 000-NET-0048) · **Constitution:** MC-11.6, MC-12.1, MC-12.3

Recorded by the network track on 2026-10-25, for the plan [docs/communications-and-agents.md](../docs/communications-and-agents.md).

## Problem

- **`tls` verifies a server** against `tlsroots.pem` on the boot disk, or by a pinned key (351-NET-0002).
- **No build puts public roots there.** The suites make their own test CA, and the updater pins its server's key.
- **So nothing on the machine can reach** `api.telegram.org`, an IMAP server, `api.anthropic.com` or any web site.

## Plan

- **The root store:** Mozilla's, as the CCADB publishes it for TLS server authentication (for example, curl's `cacert.pem`, extracted from Mozilla's). It is committed under `third_party/roots/`, with its date, its source and its SHA-256, and listed in THIRD_PARTY.md under MPL-2.0. A committed copy keeps the build reproducible.
- **`scripts/update_roots.py`** fetches a newer one, checks it parses, and shows what was added and removed. A release takes it like any other change.
- **The image builders** (`02_build.sh`'s volume, `make_usb_image.py`) put it at the root as `tlsroots.pem`. The owner may replace or trim the file, and `tls` reads it at every connection, as now.
- **What `tls` checks stays as it is:** webpki against these roots, the certificate's time from the RTC, the server's name.

## Acceptance criteria

- **In QEMU, x86:**
  - a server with a certificate under one of the store's roots is refused when its name does not match (no network to the outside is needed: the suite signs a certificate with a test root added to a copy of the store);
  - a server with a certificate under no root in the store is refused.
- **On the maintainer's machine:** a real HTTPS request to a public site verifies, named with its configuration (MC-12.1).
- **THIRD_PARTY.md and docs/network** name the store, its source, date and licence.

## Related

[351-NET-0002](../issues-done/351-NET-0002-https-for-programs.done), [000-NET-0047](000-NET-0047-http-for-api-clients.md), [000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md).
