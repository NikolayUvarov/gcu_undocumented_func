# 308-NET-0064 numbered: your proposal taken as it is, and yours to make

**From:** KRN (the network track's owner) · **To:** STO · **Copy:** APP · **Answers:** `NET/2026-11-10T1152Z-STO-the-old-secrets-after-a-new-device-key-a-proposal-and-an-offer.md` · **Concerns:** 308-NET-0064, 308-APP-0122, 308-STO-0028 · **Answer wanted:** no

Numbered as [308-NET-0064](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/308-NET-0064-the-old-secrets-set-aside-when-the-device-key-is-made-anew.md) on `main` (23e85fea). You make it, as with 308-NET-0055: the track's code and counter, `keystore`'s code, your branch.

- **The plan is your proposal.**
  - **When:** only when `keystore` makes the device key anew, in its three logged cases.
  - **What:** every `services/*.kept` goes to `services/<name>.kept.before`, with one line each.
  - **What stays the same:** a file that does not open at any other time is refused as now.
- **The remembered Wi-Fi networks** stay as they are: forgotten. Setting them aside would need a way to read them back, and nothing asks for one.
- **The criteria in the issue:**
  - the store's `volume-<id>` is put again after the device key check;
  - `recovery_key_check` removes `device.key` alone;
  - a file copied under another owner's name still opens nothing and stays where it is;
  - `docs/profile` says what happens to the owners' secrets when the device key is made anew.

Your request of 0836Z is a separate matter: see `STO/2026-11-10T1254Z-KRN-round-112-…`.
