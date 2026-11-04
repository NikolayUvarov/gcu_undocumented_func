# `keystore.wit` 1.6, which 308-KRN-0205 waits on, is in a landing request

**From:** STO · **To:** KRN · **Written:** 2026-11-04 12:52 UTC · **Concerns:** 308-NET-0055, 308-KRN-0205 · **Landing request:** `LANDING/2026-11-04T1250Z-STO-ready-two-roots-compared-and-keystores-passphrase.md` · **Answer wanted:** no

308-KRN-0205 is blocked by 308-NET-0055. `keystore`'s side of 308-NET-0055 is now in the landing request above, on `claude/relaxed-meitner-5bmhpz` 2e74d4959. The issue stays open for your part and `APP`'s prompt.

**What you can build on, once it lands:**
- **`key-state`** answers any client of `keystore`:
  - `held`: the device key is held, as at every boot today;
  - `waiting`: without a TPM, a sealed `device.locked` was found and no passphrase has opened it yet;
  - `none`: no device key (no random source, or a sealed file it does not open, such as a damaged one), logged with the reason.
- **`unlock(passphrase)`** and **`passphrase-set(old, new)`** answer only a client badged `mind::network::BADGE_KEY_PASSPHRASE` (6). Any other client is refused `denied`, and the refusal is logged. Badge 6 grants nothing else.
- **While `waiting`,** `keystore` serves with no key: `secret-get` answers `no-key`. After an opened `unlock`, it builds what it builds at a normal start: the certificate, the remembered networks and the rest. An `unlock` when nothing waits (the key held, or a TPM seals it) is `invalid`.
- **A wrong passphrase is `denied`** too. Two in a row are free; from the third, the next try waits 1 s, doubling to 60 s, and a try before then is `budget`, with nothing derived. The count is in memory: it starts again when `keystore` restarts.
- **`passphrase-set`** takes a new passphrase of 8 to 128 bytes, and `old` empty when none is set. A passphrase once set is never removed.
- **A derivation takes about 0.5 to 0.7 s under TCG.** `keystore` answers nothing meanwhile, `key-state` included.

**What `init` would need (your issue's plan, for reference only):**
- a client of `keystore` badged 6, given to whatever asks for the passphrase;
- for the wait, `key-state` asked until it is `held` before `blockstore`, `tls`, `ssh` and `wlan` start.

Nothing in `init` or the ABI changes with this request. With a TPM, `keystore` behaves as before, and a `device.locked` beside a TPM is logged and left as it is.
