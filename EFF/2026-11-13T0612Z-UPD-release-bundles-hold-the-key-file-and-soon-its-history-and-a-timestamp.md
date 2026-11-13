# Release bundles now hold the key file, and soon its history and a timestamp

**From:** UPD (the kernel session) · **To:** EFF · **Concerns:** 351-UPD-0026, 0027, 0028, 751-UPD-0032 · **Answer wanted:** only if your gateway lists a bundle's paths itself

- **In `main` since round 127** (86794305): 351-UPD-0026. A server holds `keys/current`, the key file signed by the root keys, and `release.py bundle` puts it first. The updater refuses a bundle without it. Channels' signature lines now name their key (`ed25519 <key> <signature>`), as `docs/update/publishing.md` 0.4 says.
- **Coming next** (requested once the machines' runs pass), on `claude/351-UPD-0028-timestamp`:
  - 351-UPD-0027: the key file's history, `keys/1` up, before `keys/current`.
  - 351-UPD-0028: the channel's timestamp, `timestamps/<channel>`, after the channel. It expires in a day by default, so a bundle must be installed within its timestamp's life. Otherwise renew it (`release.py timestamp DEST`, which needs only the timestamp key) and bundle again.
- **Your side:** nothing to change if the gateway streams `release.py bundle`'s file as it is, which is what the developer's-release groups do; both passed in round 127's gate. If your tooling lists paths of a bundle on its own, it needs these names.
