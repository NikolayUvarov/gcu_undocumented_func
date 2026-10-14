# 550-DRV-0028 — `bcm_wifi`: transmit power controlled from the SPROM, and answers on every channel

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

Stage 5 ([550-DRV-0026](../issues-done/550-DRV-0026-bcm4331-transmits.done)) sent a probe request on each of channels 1 to 11 at a fixed, low gain (entry 80 of the gain table) with power control off. Every one was sent, and three probe responses came back on channel 1; none came on channels 2 to 11, where networks sent beacons at −39 to −54 dBm. A station needs its frames heard on any channel, at a power that is known and within what the SPROM allows.

## Plan

- **The SPROM's power data** (revision 9): the per-core 2.4 GHz maximum power (quarter dBm) and PA parameters at byte bases 0xC0, 0xE0 and 0x100, and the board flags, read and logged.
- **Power control as intended,** not as b43 writes it: the idle TSSI measured with sample playback, the 64-entry TSSI-to-power estimate per core from the PA parameters, the target power at most the SPROM's maximum, hardware power control on. The fact sheet's defects D1 to D8 in b43 are not copied.
- **The answers per channel:** the probe request's time and the wait for answers logged per channel; a second probe request after a longer wait; the gain or the target power raised in steps only while no answer comes and the SPROM's maximum is not reached.

## Acceptance criteria

On the MacBook Pro, one boot on request logs the SPROM's power data, the idle TSSI per core, power control on with its target, and probe responses on most channels with a network that answers probes. The power used never passes the SPROM's maximum. Evidence for this machine only (MC-12.1).

## Progress

**2026-10-13, the maintainer's run of `fast-test` 28d9c3d** (`log:boot0004.log`, stage 4 through the radio's calls of 550-DRV-0027):

| Test | Gain entry | Result |
|---|---|---|
| Broadcast probe requests, channels 1 to 11, at 1 and at 6 Mbit/s | 80 | all 22 sent once; no probe response |
| Directed probe requests, 7 tries each: `TP-LINK_POCKET` (channel 6, −38 dBm), `Xiaomi_7C34` (channel 6, −39 dBm), `David_2.4G` (channel 1, −41 dBm) | 80 | none acknowledged |
| The same | 40 (b43's) | `David_2.4G` acknowledged at the first try and answered; the two on channel 6 not acknowledged |

- **What it shows.** At entry 80 (`10090010`: the baseband multiplier at 0x10) the radio is too weak to be heard. At entry 40 (`10190033`) an access point on channel 1 hears it. Entry 40 is now the service's gain.
- **Still open.** The networks on channel 6 heard nothing, though they are the strongest and are received well there. This is the third run in which only channel 1 has answered.
- **The next run tells these apart:** the channel, the access point, the rate or the gain. It sends three directed probe requests to every network heard (channels 1, 3 and 6 here), at entries 40 and 20 and at 1 and 6 Mbit/s.

**2026-10-13, the second run, `fast-test` 29fee23** (`log:boot0001.log`, gain entry 40): directed probe requests, 3 to each network, up to 7 tries each.

| Network (channel, signal) | 1 Mbit/s CCK, entry 40 / 20 | 6 Mbit/s OFDM, entry 40 / 20 |
|---|---|---|
| `ZATV` (8, −39 dBm) | 3/3, 3/3 acknowledged | 3/3, 3/3 |
| `CoolNet2` (8, −40 dBm, the same device) | 2/3, 3/3 | 2/3, 2/3 |
| `TP-Link_5EB3` (13, −41 dBm) | 3/3, 3/3 | 0/3, 0/3 |
| `Tenda_E25140` (11, −42 dBm) | 2/3, 2/3 | 0/3, 0/3 |
| `David_2.4G` (1, −44 dBm) | 3/3, 3/3 | 0/3, 0/3 |
| `TP-LINK_POCKET` (6, −32 dBm) | 0/3, 0/3 | 0/3, 0/3 |

Broadcast probe requests at 1 Mbit/s were answered on channels 1, 2, 8 and 11: 20 probe responses, and 9 networks in all.

- **CCK works on every channel tried.** At entry 40, 1 Mbit/s frames are acknowledged by five of the six networks, on channels 1, 8, 11 and 13. The "channel 1 only" of the earlier runs came from the gain at entry 80.
- **Entry 20 adds nothing visible.** Entry 40 stays.
- **The pocket router on channel 6 acknowledges nothing at any rate or gain**, though it is the strongest. Its beacons are received. It is one access point's behaviour, not the channel's: no other network on channel 6 was among the six probed.
- **OFDM is the defect.** Only one device acknowledges 6 Mbit/s frames, and four that acknowledge CCK ignore them. The frames go out (tries counted, not suppressed), so their content or quality is wrong for most receivers. The suspects are, in order:
  - the TX IQ/LO calibration, which is not done (OFDM is far less tolerant of it than CCK);
  - the OFDM PLCP or PHY control words;
  - the per-rate power.
- **Until OFDM is fixed** the station should send at CCK rates only (1, 2, 5.5, 11 Mbit/s): told to `NET`.

**2026-10-14, the next run.** The OFDM fact sheet (b43, brcmsmac, described) finds our OFDM header equal to b43's field for field. What it does point to:
- the core field 0x00C0 is a bitmap of cores 0 and 1 in brcmsmac's reading;
- b43 sends OFDM through both cores in SISO mode, the same signal without cyclic delay, which cancels towards some receivers;
- Broadcom's driver uses CDD (PHY control 1, mode 1) on several cores.

Stage 4's last step now sends 3 directed probe requests to each network in eight ways:
- 1 Mbit/s on cores 0+1, and on core 0;
- 11 Mbit/s on cores 0+1 (a power check: a weak signal hurts it too);
- 6 Mbit/s on cores 0+1, on core 0, on core 1 and on core 2 alone;
- 6 Mbit/s on cores 0–2 with CDD.

It also logs the PHY registers that decide OFDM reception (0x0B0, 0xC01, 0xC0A, 0x17E).

**2026-10-14, the third run, `fast-test` eae3690** (`log:boot0001.log`, gain entry 40): three directed probe requests per way of sending, acknowledged of 3.

| Network (channel, signal) | 1M, cores 0+1 | 1M, core 0 | 11M | 6M, cores 0+1 | 6M, core 0 | 6M, core 1 | 6M, core 2 | 6M, cores 0–2 CDD |
|---|---|---|---|---|---|---|---|---|
| `David_2.4G` (1, −37) | 3 | 3 | 3 | 1 | 2 | 1 | 2 | 0 |
| `un2` (1, −41) | 3 | 3 | 3 | 2 | 3 | 1 | 3 | 3 |
| `Tenda_E25140` (3, −41) | 2 | 3 | 3 | 1 | 0 | 1 | 1 | 0 |
| `CoolNet2` (8, −38) | 3 | 3 | 3 | 1 | 1 | 3 | 3 | 2 |
| `ZATV` (8, −38) | 3 | 3 | 3 | 3 | 3 | 3 | 2 | 2 |
| `TP-LINK_POCKET` (6, −30) | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |

- **CCK at 1 and 11 Mbit/s is nearly always acknowledged at the first try.** So the power is enough for CCK.
- **OFDM now gets through to every network but the pocket router, with many tries.** No core and no CDD is clearly better: the cores are not the cause.
- **The PHY registers read as expected:** classifier `0DF7` (CCK and OFDM on), `C01` 0000, `C0A` 0100, `17E` 3830.
- **The pocket router acknowledges nothing,** CCK at 11 Mbit/s included. That is the router's behaviour.
- **What OFDM depends on that CCK does not:**
  - the baseband multiplier (table 13), which scales OFDM only;
  - and with it how the gain is split between the radio and the baseband (PA compression of OFDM's high peaks, or LO leakage at a high radio gain with a low multiplier);
  - b43 runs no TX IQ/LO calibration either.
- **The next run** sends 1 and 6 Mbit/s in five splits (radio gain word / multiplier): 1019/33 (entry 40), 1019/20, 1019/48, 1009/40, 1041/20.

## Related

[550-DRV-0026](../issues-done/550-DRV-0026-bcm4331-transmits.done), [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md), `bcm_wifi/`.
