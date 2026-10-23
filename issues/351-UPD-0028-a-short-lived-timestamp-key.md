# 351-UPD-0028 — A short-lived timestamp key for the channel

**Type:** update (keys) · **Owner:** `UPD` track · **Priority:** P2 · **Status:** open · **Blocked by:** [351-UPD-0026](351-UPD-0026-root-keys-and-a-signed-key-file.md) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.4, MC-9.6

Split from [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) (the maintainer, 2026-10-23).

## Problem

A channel's expiry is set when the channel is published, for days or weeks. So a server, or anyone holding an old file, can freeze a device on the latest signed channel until it expires (Appendix B.5). And renewing the expiry needs the release key wherever the channel is published.

## Plan

- **A timestamp file** holds:
  - the channel file's SHA-256 and its version;
  - a short expiry (hours to a day).

  It is signed by the timestamp key the key file names. That key is kept on the publishing server; the release key is not.
- **The updater fetches the timestamp first** and refuses it if:
  - it has expired;
  - its version is below the last one seen;
  - it names another channel digest.

  Then it fetches the channel the timestamp names.
- **Publishing:** `scripts/release.py timestamp` renews it, and a publish writes it last.
- **The offline policy** ([351-UPD-0025](../issues-done/351-UPD-0025-expiry-and-the-offline-policy.done)) applies with the shorter expiry.

## Acceptance criteria

- Host tests of the format.
- QEMU `update` suite, x86 and aarch64, each refused with no record written:
  - an expired timestamp;
  - a timestamp signed by the release key;
  - a timestamp naming another channel digest;
  - an older timestamp.
- Renewing the timestamp without the release key keeps the device updating.
- docs/update/publishing.md and publishing_RU.md give the procedure.

## Related

[351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0025](../issues-done/351-UPD-0025-expiry-and-the-offline-policy.done), [351-UPD-0026](351-UPD-0026-root-keys-and-a-signed-key-file.md).
