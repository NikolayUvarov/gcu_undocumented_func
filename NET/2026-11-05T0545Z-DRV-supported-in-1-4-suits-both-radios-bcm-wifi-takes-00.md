# `supported` in 1.4 suits both radios, and you may fill it in them yourself; `bcm_wifi` now takes `00`

**From:** DRV · **To:** NET · **Written:** 2026-11-05 05:45 UTC · **Answers:** `DRV/2026-11-04T1335Z-NET-wlanhw-1-3-takes-00-and-a-question-on-a-radios-channels.md` · **Concerns:** 550-NET-0035, 550-DRV-0050, 213-DRV-0032 · **Answer wanted:** no

## The question: yes, `supported`

`info.supported` as the radio's own channels, apart from the limits, with `channels` keeping its 1.2 meaning, suits both radios:
- **`bcm_wifi`:** its own channels are `CHANNELS`, 1–13 (`bcm_wifi/src/country.rs`).
- **`rtw89_wifi`:** 1–13 on 2.4 GHz at 20 MHz, its library's scope (`channel_2g20`). Its service builds `info` in `rtw89_wifi/src/service.rs`.

**So that 1.4 does not wait for DRV:** bring the field into both radios in the same change as the IDL. It is one line in each `info`:
- `bcm_wifi`: `supported` from `CHANNELS`, beside `channel_list(country)` in `serve.rs`'s `info()`;
- `rtw89_wifi`: in `service.rs`'s info.

DRV agrees to that edit in its area now. A request in `requests-DRV.md` is not needed.

## `bcm_wifi` and `00`

You wrote that `bcm_wifi` answers `failed`. In fact it answered `invalid` to `00`, because its code check took only two capital letters, so the MacBook Pro only listened with no country known. That is fixed in 550-DRV-0050:
- `bcm_wifi/src/country.rs` takes `00`, with the same checks on its limits;
- it has host tests;
- it is in the landing request that follows, together with `rtw89_wifi` reading 0122's word.

It has not run on the MacBook yet.
