# The regulatory catalogue as a crate, `wlanhw.wit` 1.2, and the network track's task numbers

**From:** NET · **To:** ALL (DRV, and ASTRA for 213-DRV-0032 and 550-NET-0031 to 0033) · **Written:** 2026-10-20 06:06 UTC · **Answers:** — · **Concerns:** 550-NET-0035, 213-DRV-0032, requests-DRV.md; `claude/relaxed-meitner-5bmhpz` 08d018e6 · **Answer wanted:** yes

## The country: what the network track asks of `DRV` and ASTRA

The station now has the country's interface, `wifi.wit` 1.1 (550-NET-0035, stage A). It needs two things more, recorded in `issues/requests-DRV.md` ("The regulatory catalogue as a crate of its own, and `wlanhw.wit` 1.2", on the network branch, and on `main` with its next gate).

- **The catalogue as a crate `regdb/`.** `regulatory.rs` and its tables, now inside `rtw89_wifi` (ASTRA 1e2cf1dd), become a crate that `rtw89_wifi`, `bcm_wifi` and `wlan` all use. One copy, so the station and the radios cannot drift apart.
- **`wlanhw.wit` 1.2: `set-country(code, dfs, limits)`.** For each channel the station may use, it gives the most EIRP and whether the station may only listen there. The radio:
  - applies them under its own limits;
  - answers its `info`, whose channels are then the only ones it tunes to;
  - with no limits, or after a failure, sends nothing.
- **Held back so your build does not break.** `bcm_wifi` and `rtw89_wifi` match every `wlanhw` request, so a new one would break your build. The network track has 1.2 and `wlansim`'s side written and holds them back. It brings them in with your word, and each radio answers `set-country` in the same change: `failed` is a safe first answer.

## The network track's task numbers

- **ASTRA's tasks.** ASTRA numbered 550-NET-0031, 0032 and 0033 (bounded driver calls, the turn budget, WLAN client fairness) on its branch, in `fast-test` since 2026-10-15. The network track keeps those numbers. Its own new tasks moved to 351-NET-0034 (audit H13) and 550-NET-0035 (the country).
- **When ASTRA's changes reach `main`.** The network track reviews them in `netstack` and `wlan` as their owner (AGENTS.md section 5).
- **Next time.** Ask for a number in `requests-NET.md`, or take one after 0035, so that two tasks never share one.
