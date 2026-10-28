# 000-NET-0046 — Public root certificates in the image

**Type:** network (`tls`'s root store, the image builders) · **Owner:** `NET` · **Priority:** P1 · **Status:** open (landed in round 39; a real machine's HTTPS left) · **Blocked by:** — · **Main task:** — (for 253 to 261 and 000-NET-0048) · **Constitution:** MC-11.6, MC-12.1, MC-12.3

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

## What was done (2026-10-28)

- **The store:** `third_party/roots/cacert.pem`, curl's extract of Mozilla's roots.
  - Mozilla's data as of 2026-10-20: 118 roots, 200 116 bytes.
  - Its record is `third_party/roots/SOURCE.md`: source, date, count, size and SHA-256.
  - The licence is MPL-2.0, with its text in `LICENSES/MPL-2.0.txt`.
- **`scripts/update_roots.py`:**
  - **Fetching a newer store.** It fetches one, checks it against the SHA-256 curl publishes beside it, and checks that every certificate is one DER certificate whose SHA-256 is the fingerprint written above it. It also checks that the store fits the 512 KiB `tls` reads. It then shows the roots added and removed, and writes the store and its record.
  - **With `--check`,** it checks the committed store against its record.
- **The builders ship the store as `tlsroots.pem` at the volume's root.** `02_build.sh` and `scripts/build_aarch64.sh` run `--check` first, and `scripts/make_usb_image.py` packs the file.
  - The file stays at the root when the image has slots.
  - The boot manifest does not list it: the owner may replace or trim it.
- **`tls` counts what it took.** Its `READY` line names the roots it parsed from the file, and how many did not parse, if any.
- **The `tls` and `net` suites' store** is the public one with the test CA added, as an owner adds a root.
  - The `tls` suite requires all of them parsed: 119 on this store.
  - It keeps its refusals: a name that does not match (`NotValidForName`) and a CA that is not in the store (`UnknownIssuer`).
- **THIRD_PARTY.md, `docs/network/downloads.md`, `docs/profile/tcb.md` and `docs/communications-and-agents.md`** name the store.
- **Tested in QEMU under TCG, 4 CPUs:**
  - **x86:** the `tls` and `net` suites passed, `tls` reading `119 ROOTS FROM tlsroots.pem`;
  - **aarch64:** the same suites passed, and `scripts/build_aarch64.sh` checked and shipped the store.

## Acceptance criteria

- **In QEMU, x86:**
  - a server with a certificate under one of the store's roots is refused when its name does not match (no network to the outside is needed: the suite signs a certificate with a test root added to a copy of the store);
  - a server with a certificate under no root in the store is refused.
- **On the maintainer's machine:** a real HTTPS request to a public site verifies, named with its configuration (MC-12.1).
- **THIRD_PARTY.md and docs/network** name the store, its source, date and licence.

## Progress (2026-10-28)

Landed in round 39 (`main` 8721ef86, the gate on its code 381d1e85): Mozilla's public roots as `tlsroots.pem` in both images, checked against their record at each build; the `tls` suite on x86 and aarch64 in the gate. Left: HTTPS to a public server from a real machine, with the build named (MC-12.1).

## Related

[351-NET-0002](../issues-done/351-NET-0002-https-for-programs.done), [000-NET-0047](000-NET-0047-http-for-api-clients.md), [000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md).
