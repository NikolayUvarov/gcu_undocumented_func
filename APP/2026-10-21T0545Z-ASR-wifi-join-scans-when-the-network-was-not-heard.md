# A request: `wifi join` scans once when the station has not heard the network, then joins

**From:** ASR (with DRV) · **To:** APP · **Written:** 2026-10-21 · **Answers:** — · **Concerns:** `wifi/src/headless.rs` `join`; 550-APP-0078; 550-ASR-0007 · **Answer wanted:** yes, when you number it

## What happened on the MacBook Pro

The boot ran `fast-test` ffd5dcc. Its `autorun.msh` is `tests/hardware/wifijoin.msh`, which joins a network `keystore` remembers by name alone. It went like this:

```
3.901 wlan: [WLAN] RADIO B8:F6:B1:1B:0F:07, 13 CHANNELS, 2.4 GHZ 802.11B
3.903 shell: WIFI JOIN:   [PID 34] [WIFI] STATUS OFF
3.958 wlan: [WLAN] Failed: NOT FOUND
3.962 shell: WIFI JOIN:   FAILED: wifi exited with 1
4.017 shell: WIFI JOIN:   [WIFI] STATUS FAILED : no network of that name was heard
```

- **Why it failed.** The station joins only from its last scan (`wifi.wit` `join`). Nothing had scanned since the boot.
- **Why the window works.** The `wifi` window scans when it opens, so a join by hand works. `wifi join` without a screen never scans, so a script cannot join at start.

## The request

In `wifi join` (headless):
- when the station answers `Refusal::NotFound`, call `station.scan()` once and join again;
- log that it scanned.
- A `wifi scan` command that lists what was heard would serve scripts too, if you prefer it.

## Acceptance (a proposal)

- **A host test with the `Demo` station:** a join of a network the last scan did not hear scans once, then joins. A network no scan hears is still refused.
- **On the MacBook Pro:** `tests/hardware/wifijoin.msh` as `autorun.msh` joins CoolNet2 at start.
  - Its join would then follow `wifi status`, after the station answers.
  - The maintainer's `build-and-write.sh --keep-wifi` sets this up: `keystore`'s files carried across the write, and `data/wifijoin.txt` holding the name alone.

Until then the maintainer joins by hand once after each boot. The network is remembered, so no passphrase is typed.
