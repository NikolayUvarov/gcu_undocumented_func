# 351-UPD-0028 — A short-lived timestamp key for the channel

**Type:** update (keys) · **Owner:** `UPD` track · **Priority:** P2 · **Status:** in progress · **Blocked by:** [351-UPD-0027](351-UPD-0027-key-rotation-and-revocation.md), on whose branch it is made · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.4, MC-9.6

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

## Progress

**Made (2026-11-13)** on `claude/351-UPD-0028-timestamp`, on 351-UPD-0027's branch:
- **The format** (`libmind/src/release.rs` `Timestamp`, `parse_timestamp`, `encode_timestamp`, `timestamp_is`; `idl/parse.wit` 4.1 `timestamp`, which the parser service answers): `{"channel":…,"digest":…,"expires":…,"version":…}` and the timestamp keys' signature lines. `version` is the timestamp's own, raised by each renewal; the plan's "its version" read so, since the channel's version is in the channel the digest names.
- **The device** (`updater`): after the key file, it reads `timestamps/<channel>` and refuses it when the timestamp role's threshold did not sign it, when it is another channel's, when its version is below the highest taken (kept in `system/updater/timestamp`), or when it has expired; then it takes only the channel file it names by SHA-256. The channel's expiry counts as the shorter of its own and the timestamp's, in the log, `metadata` and `apply` (the offline policy of 351-UPD-0025).
- **The publisher** (`scripts/release.py`): `publish` writes the timestamp last, 24 hours by default; `timestamp DEST` renews it with `$MIND_TIMESTAMP_KEY` (the test timestamp key without it), never the release key; `check` verifies it; `bundle` holds it.
- **The layout:** `tools/httpd` and `mind::http_serve` serve `timestamps/<channel>` fresh, as a channel; `scripts/link_update.py` puts it after the channel.
- **Docs:** `docs/update/publishing.md` and `publishing_RU.md` 0.6 (the procedure), `updater.md` and `updater_RU.md` 0.12, the profile's Article 9 row, `docs/idl/README.md`.
- **Tested** (this container): `tests/release_host.rs` (the format: read and encoded again, other forms refused, any byte changed refused or read as what it encodes, a lying parser caught); `tests/release_test.py` (renewed without the release key, one more version; expired, the release key's and another channel file's refused by `check`); `tests/updater_host.rs`, `tests/http_serve_host.rs`, `tests/idl_fuzz_host.rs`.
- **The QEMU checks** (`tests/qemu_smoke.py`, the `update` suite, x86 and aarch64): an older timestamp, an expired one, one the release key signed and one naming another channel file, each refused with no record written; then the timestamp renewed by `release.py timestamp` with no release key, after which release 6 is fetched and booted as before. Every channel the suites serve gets its timestamp (`serve_channel`). Runs on the maintainer's machines: below, once made.

## Related

[351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0025](../issues-done/351-UPD-0025-expiry-and-the-offline-policy.done), [351-UPD-0026](../issues-done/351-UPD-0026-root-keys-and-a-signed-key-file.done).
