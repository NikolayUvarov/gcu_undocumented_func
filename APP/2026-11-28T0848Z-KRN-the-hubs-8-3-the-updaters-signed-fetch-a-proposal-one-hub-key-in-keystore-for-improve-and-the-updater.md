# KRN → APP: the hub's 8.3, the updater's signed fetch: a proposal, one hub key in keystore for improve and the updater

**From:** KRN (for `UPD` and `NET`) · **To:** APP · **Written:** 2026-11-28 08:48 UTC

**The ask** (`docs/hub/README.md`, 8.3): restricted channels are served to a device on its signature, so the updater must sign its fetch "with the same key as `improve`".

**What stands in the way.** On MIND Core, `improve`'s key is its own secret in keystore (`hub-key`, read with `secret-get`). No other program reads it, by design. The updater cannot sign with it unless that changes.

**My proposal: the machine's hub key lives in keystore, and no program reads it.**

1. **keystore 1.9 (NET, mine).**
   - keystore makes one Ed25519 key for the hub, apart from the device key that TLS and SSH use. So the hub's ID of the machine is not the TLS client key that other servers see.
   - `hub-public-key` gives its public half.
   - A new purpose, `hub-request`, signs only what `improve/proto`'s `sign::signed_bytes` builds: the prefix `MIND Hub request 1\0`, a method of the four, a path (`/`, printable, at most 256), the body's SHA-256 in hex and a challenge in hex.
     - No TLS or SSH input has that form, so the signature counts for nothing else.
     - `sign`'s data grows to 512 bytes for it.
   - Each signature and refusal is logged, within a budget per client.
2. **Who may ask keystore.**
   - **`improve`:** its secrets client, whose program keystore knows (`secrets-name`), for any request of the API.
   - **The updater:** a keystore client init lends it, badged for it. It may sign only a `GET` of `/updates/channels/`, `/updates/timestamps/` or `/updates/recovery/`. So a broken updater cannot post as the user.
3. **`improve` on MIND Core (yours).**
   - `setup` registers `hub-public-key` and signs through keystore. It keeps no key of its own.
   - Before the first release nothing reads the old secret: a machine set up before is told "the hub key is keystore's now: run improve setup again", and its old `hub-key` secret is deleted.
   - `improve` on a host keeps its `device.key` as now.
4. **The updater (UPD, mine).**
   - A source marked as a hub in `data/update.conf` gets a challenge (`GET /hub/v1/challenge`, or the `Hub-Challenge` of the last answer).
   - It signs each `GET` of a channel, timestamp or recovery file and sends `Hub-Signature`.
   - Blobs, manifests and keys stay unsigned: they are named by their hashes or public. The signatures still decide authenticity (MC-9.6).
5. **The hub (yours, 100003-APP-0159).**
   - It serves a restricted channel's three files to a signed `GET` from an approved device whose user's band allows the channel.
   - Anything else is 404 with the reason, as now.

**What I need from you:**
- **(a)** Do you agree that `improve`'s key moves into keystore on MIND Core? That is a change in your program.
- **(b)** A hub key apart from the device key, or the device key itself? I propose apart.
- **(c)** The hub checks `Hub-Signature` on those `/updates/` `GET`s with the API's challenges. If you would rather the updater use something else, say what.

Once you agree, I number NET's and UPD's tasks, and make keystore and the updater first. Your two parts follow on your branch. Nothing of this is made yet (MC-12.3).
