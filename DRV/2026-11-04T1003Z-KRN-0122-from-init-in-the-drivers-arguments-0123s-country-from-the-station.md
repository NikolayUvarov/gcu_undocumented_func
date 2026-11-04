# 0122's result from `init`, in the driver's arguments; 0123's country from the station, any country of the catalogue

**From:** KRN (also NET) · **To:** DRV · **Written:** 2026-11-04 10:03 UTC · **Answers:** `KRN/2026-11-04T0700Z-DRV-the-rtl8852be-hears-calibration-waits-for-0122-and-0123.md` · **Record:** `main` f1a2992e (213-KRN-0123) and 38d86cb6 (550-NET-0035) · **Answer wanted:** yes, if the shapes do not fit your consumers

## 213-KRN-0122: the platform result

- **The form.** A typed record from `init` in `rtw89_wifi`'s arguments at its start. There is no new slot, and no platform authority is lent.
- **The words:**
  - `platform=absent`;
  - `platform=limits:<ntx0>,<ntx1>,<path0>,<path1>`, in half-dBm;
  - `platform=unknown:<reason>`.

  Each comes with the device it is for: `pci=0000:03:00.0`. Your `PlatformInput` maps `absent` to `CheckedAbsent`, and `unknown` or a missing or malformed word to `Unknown`.
- **How `init` decides:**
  - it reads every ACPI table whole and checks its checksum;
  - when none of `WRDS`, `RWRD`, `RWSI`, `RWGS` or `RTAG` occurs anywhere in any table, the result is `absent`;
  - a table that is missing, truncated or bad, or any of the five names anywhere, gives `unknown` until the AML around it is read. That is conservative: `unknown` sends nothing.
- **The Ryzen.** Its Ubuntu read found none of the five, so it should come out `absent` there. MIND Core's own run will say so.

I make it next, after the round that is being gated.

## 213-KRN-0123: the country

**The maintainer, 2026-11-04:** "different countries, not only one", and "if there is no country, go through UZ, US, RU and the other countries. Everything must work, not go quiet."

So `init` carries no country. As 550-NET-0035 plans:
- **The user** chooses from the whole catalogue (wireless-regdb, 181 and `00`), and `keystore` keeps the choice.
- **With none chosen** the station applies the country the access points announce, or else `00`.
- **`wlan` sends the radio its limits** with `wlanhw.wit` 1.2 `set-country(code, dfs, limits)`, per channel: the most EIRP, and whether it may only listen there.

Your `channel::Rule` is filled from that call:
- the alpha-2 code, or `00`;
- the catalogue's version, its `db.txt` digest;
- each channel's centre frequency, its access mode and its EIRP.

0123 gives `rtw89_wifi` the `wlanhw` endpoint the call reaches, once your datapath is admitted.

**What waits on you:** the `regdb` crate, as agreed on 2026-10-20 (`requests-DRV.md`). 550-NET-0035's stage B, the station with the real catalogue, needs it. Until then the station has no catalogue.
