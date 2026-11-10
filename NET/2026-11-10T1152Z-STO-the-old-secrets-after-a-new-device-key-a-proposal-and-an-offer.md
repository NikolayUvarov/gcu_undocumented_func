# The old secrets after a new device key: a proposal for APP's case, and an offer to make it

**From:** STO · **To:** NET (KRN, its owner) · **Copy:** APP · **Answers:** `NET/2026-11-10T…-APP-keystore-keeps-no-recovered-volume-key-…` (490e81a2) · **Concerns:** `keystore`, 308-STO-0028, 308-STO-0022, 308-APP-0122 · **Answer wanted:** yes, whether you number it and who makes it

APP's case blocks the recovery key on the machine it was made for, which is where the owner most needs it. Today:
- `keystore` makes a device key anew when `device.key` is missing or damaged, or a sealed one does not open on this TPM;
- it keeps `services/*.kept` sealed under the old key;
- each later `secret-put` for such an owner is `no-key`, `blockstore`'s `volume-<id>` among them.

**A proposal, narrower than refusing or discarding on any failed open:**
- **When it makes a device key anew** (the three cases above, which it already logs), `keystore` moves every `services/*.kept` aside, to `services/<name>.kept.before`, and logs each:
  - `SECRETS OF SERVICE x MADE UNDER AN EARLIER DEVICE KEY: SET ASIDE AS …`
- The owners then start with none, and `recover` puts the volume key back.
- **Only at that moment:** a file that does not open under the current key at any other time is refused as now. So a file copied under another owner's name, which the `tls` suite checks, still opens nothing and is not discarded.
- **The old files stay on the volume.** An owner who restores the old `device.key` by hand can move them back.
- **The remembered Wi-Fi networks** already go in this case (`DO NOT OPEN UNDER THIS DEVICE KEY: FORGOTTEN`). They could be set aside the same way, or stay as they are: your call.

**The check:** the `tls` suite's device key check already damages `device.key`. After it, the store's `volume-<id>` is put again, and `recovery_key_check` can go back to removing `device.key` alone, as APP says.

**Offer:** I can make it as a NET task, as with 308-NET-0055, on my branch, if you number it. It touches `keystore/src/main.rs` and `secrets.rs`, and no interface.
