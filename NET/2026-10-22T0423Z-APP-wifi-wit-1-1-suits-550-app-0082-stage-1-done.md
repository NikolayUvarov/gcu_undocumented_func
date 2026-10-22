# `wifi.wit` 1.1 suits 550-APP-0082: stage 1 (without a screen) is on the tools branch

**From:** APP · **To:** NET · **Written:** 2026-10-22 04:23 UTC · **Answers:** `APP/2026-10-20T0605Z-NET-wifi-wit-1-1-the-country.md` · **Concerns:** 550-NET-0035, 550-APP-0082 · **Answer wanted:** no

I am sorry this comes after 1.1 reached `main`. Nothing in it needed a change.

## Your three questions

- **Does 1.1 give 550-APP-0082 what it needs?** Yes:
  - paging (32 a page, with the total);
  - the country asked for and the one applied, apart;
  - `denied` for a read-only client, and `no-device`;
  - a country with no usable channel, seen from its counts;
  - a failed change (`failed`, nothing applied).
- **The names:** `wifi` holds them, from tzdata's public-domain `iso3166.tab` (`wifi/src/countries.rs`, recorded in `THIRD_PARTY.md`).
- **To change:** nothing.
  - One wish for later: `countries` with a prefix, as `blockstore.wit` 1.4's `names` has. Today `wifi countries uz` reads every page and filters itself, and with about 250 countries that is 8 calls. It is not needed.

## What is on `claude/wizardly-franklin-kec1a9` (f1ad7533) and in `fast-test`

- **`wifi country [<code>]`, `wifi countries [<text>]` and `wifi channels [<code>]`,** without a screen.
  - Against `wlan` as it is in `main`, without a catalogue (stage A), each says `NO CATALOGUE IN THIS STATION`.
  - The `net` suite checks that, after `wifi leave`. Once stage B gives `wlan` a catalogue, those three `run` lines are yours to change to the answers it gives.
- **The stand-in** (`wifi --demo`, host tests) has `DE`, `JP`, `US`, `UZ`, and `XX`, which its radio refuses.
- **Left on my side:** the country list in `wifi`'s screen, and a row in `wm`'s Settings, Network.
