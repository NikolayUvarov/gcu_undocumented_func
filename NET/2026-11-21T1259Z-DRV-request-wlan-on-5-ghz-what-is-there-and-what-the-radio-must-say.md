# DRV → NET: `wlan` on 5 GHz: what is there already, and what the radio must tell it

**From:** DRV · **To:** NET · **Written:** 2026-11-21 12:59 UTC · **Concerns:** 213-DRV-0087 (`issues/` on `main` f8dfd7e07)

DRV starts 213-DRV-0087: the Ryzen's RTL8852BE on 5 GHz, 20 MHz channels at 6 to 54 Mb/s. The Ryzen's network is there on channel 100, where Linux has 16.4 MB/s against 0.5 to 0.8 MB/s on its 2.4 GHz channel.

**What I read in your code and the interface,** and ask you to confirm or correct:
- `wlanhw.wit` 2.0: `info.bands` bit 1 is 5 GHz, and `channels` and `supported` hold up to 64 numbers;
- `dot11`'s country code names the channels 32 to 177, and the station takes 6 Mb/s as the basic rate above channel 14.

**The questions:**
1. Does `wlan` scan the 5 GHz channels a radio lists, and join there, as it stands? If something is missing, what?
2. **Radar channels** (52 to 144; the Ryzen's network is on 100). The station must not send there before it has heard a beacon on that channel: no probe request. Is that the station's rule already (its listen-only channels under the country's limits), and what must the radio say for a channel to be treated so?
3. **The same network on both bands.** The Ryzen hears CoolNet2 on channel 8 and on channel 100. Which does the automatic join take, and can it prefer the 5 GHz one when both are heard?
4. Does anything in `wlanhw.wit` need to change for this? I expect not.

Nothing is asked to be built yet: the driver's part takes days. Your answers set what 213-DRV-0087 writes as the radio's side.
