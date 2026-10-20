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

- **2026-10-20: a country's EIRP limits wait for this task.** `bcm_wifi` takes `wlanhw` 1.2's `set-country` and keeps to its channels and listen-only channels (550-DRV-0027). It does not apply the EIRP figures: its gain is fixed and its output in dBm is unknown. When power control works, the target power per channel becomes the least of the SPROM's maximum and the country's EIRP, less the antenna gain the SPROM gives.

The work on receiving OFDM moved to [550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md) on 2026-10-14. It is not part of this task's plan, and its record stays here up to then. This task keeps the transmit power: nothing of the SPROM's power data or of power control is done yet, and the runs used the fixed gain entry 40 with power control off.

The code these runs tested is on `claude/ASR-DRV` and `fast-test`, and reaches `main` after its gate: gain entry 40, the directed-probe tests, core 2's front end off, and the radio's calls of 550-DRV-0027. Until then `main`'s `bcm_wifi` has 550-DRV-0026's stage 5.

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
- **OFDM is the defect.** Only one device (`ZATV` and `CoolNet2`, two networks of one access point) acknowledges 6 Mbit/s frames, and three that acknowledge CCK ignore them (`TP-Link_5EB3`, `Tenda_E25140`, `David_2.4G`). The frames go out (tries counted, not suppressed), so their content or quality is wrong for most receivers. The suspects are, in order:
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

**2026-10-14, the fourth run, `fast-test` 04460d4** (five splits of the gain between the radio word and the baseband multiplier):
- **No split changes the picture.** CCK at 1 Mbit/s is acknowledged in every split. 6 Mbit/s is acknowledged by `ZATV` and `CoolNet2` (channel 8) in every split, and almost never by `David_2.4G`, `un2` (channel 1) and `Tenda_E25140` (channel 3).
- **The networks that do not acknowledge OFDM still answer it.** `un2` sent 3 to 5 probe responses to OFDM requests it never acknowledged, `Tenda` 1 to 2, `David` 1. They receive our OFDM frames. What fails is our side of the acknowledgement: their ACK, sent at an OFDM rate, is not taken by the microcode in time, or not received at all.
- **The cause moves from transmitting to receiving OFDM acknowledgements**: OFDM reception, or the microcode's ACK timeout for OFDM (SIFS and the 2.4 GHz signal extension).
- **The next run keeps control frames for the host (MAC control bit 22).** It counts the ACKs to us by modulation against the transmit status, and every good frame by modulation in the scan.

**2026-10-14, the fifth run, `fast-test` 180a6a1** (control frames kept; the ACKs the host saw counted by modulation): two requests at each rate, three times each.

| Network (channel, signal) | 1 Mbit/s: acknowledged; ACKs seen as CCK | 6 Mbit/s: acknowledged; ACKs seen as OFDM; probe responses |
|---|---|---|
| `TP-LINK_POCKET` (now 8, −31 dBm) | 3, 3; 3, 3 | 3, 3; 3, 3; 3, 3 |
| `CoolNet2` (8, −40) | 3, 3; 3, 3 | 1, 0; 1, 0; 7, 6 |
| `ZATV` (8, −40) | 3, 3; 3, 3 | 0, 0; 0, 0; 7, 7 |
| `David_2.4G` (1, −42) | 3, 3; 3, 3 | 0, 0; 0, 0; 0, 1 |
| `un2` (1, −43) | 3, 3; 3, 3 | 0, 0; 0, 0; 4, 3 |
| `Tenda_E25140` (3, −44) | 3, 3; 3, 3 | 0, 0; 0, 0; 0, 0 |

- **Every acknowledgement the microcode counted reached the host, and no other.** So the ACK timeout is not the cause: an OFDM ACK either is received and counts, or is never received at all.
- **Receiving OFDM is the defect.**
  - Most access points answer our 6 Mbit/s requests with probe responses (CCK): `CoolNet2` and `ZATV` 6 or 7 times, `un2` 3 or 4. They received our frames and sent an OFDM ACK. `David_2.4G` answered once and `Tenda_E25140` never, so for those two it is open whether our frames reached them.
  - We decode that ACK only from the strongest access point (−31 dBm, all of them), rarely at −40 dBm, never at −42 to −44 dBm. CCK is received at all these levels.
  - In the scan, not one OFDM frame was received on any channel.
  - OFDM reception is therefore at least 10 dB less sensitive than it should be.
- **The pocket router acknowledges everything now that it is on channel 8.** In four runs on channel 6 it acknowledged nothing, CCK included. Whether channel 6 or the router's state then was the cause is open: the other network on channel 6 (`Xiaomi_7C34`, −44 to −47 dBm) did answer broadcast requests there.
- **Next:** the HT-PHY's receive side, compared with b43's sequence register by register (stage 3: init values, tables, AGC and clip settings, channel 6's entries).

**2026-10-14, the receive side compared with b43** (an audit of facts, kept outside the repository):
- **Stage 3 reproduces b43's HT path register for register, value for value and in order.** That covers every PHY table, the 2059 init table and the channel table (channel 6 included).
- **The one gap is b43's six TX power steps,** which we leave out.
- **b43's HT path itself writes no AGC settings and runs no RSSI, RX IQ or DC calibration.** The weak OFDM reception may be b43's as much as ours.

The next run, to find where OFDM frames are lost:
- frames counted by modulation and outcome (good, bad FCS, bad PLCP), with bad frames kept for the host;
- 6 Mbit/s with each core's front end switched off in turn;
- the good OFDM frames' per-core powers and clip bits;
- PHY tables 0, 1, 2 and 0x28 at offsets 8–11 (b43 writes 0, 1 and 0x28, and doubts 0x28 against 2);
- the SPROM's board flags, chains and front-end words.

**2026-10-14, the sixth run, `fast-test` a36d9a7:**
- **Step 6 stalled.** With control frames and bad frames kept (MAC control bits 21, 22, 23), the first send got no transmit status. After that the MAC no longer suspended, so the step's other parts were skipped.
- **Suspects.** The bad-frame bits, or the reads of PHY tables 0/1/2/0x28 made with the MAC running. The microcode uses the same table address registers.
- **The next run avoids both.** Bad frames are not kept, the tables are read with the MAC suspended, and when the MAC does not suspend its state is logged.
- **SPROM, as read:**
  - board flags `1201` / `1040` (bytes 0x84, 0x86);
  - chains `0077` (byte 0xA2: three for TX, three for RX);
  - front end `2449` (byte 0xAE).
- **PHY tables at offsets 8–11.** Tables 0, 1 and 0x28 hold 9, 14, 19, 24 (b43's writes); table 2 holds 0, 1, 2, 3. If table 2 is the third core's gains (b43 doubts its own 0x28), core 2's gain steps are wrong, which would hurt OFDM, which uses every core.
- **The next run** sends 1 and 6 Mbit/s with table 2 given b43's values, then 6 Mbit/s with each core's front end off.
- **In the scan:** 0 OFDM frames on every channel again; `TP-LINK_POCKET` is on channel 6 again.

**2026-10-14, the seventh run, `fast-test` 562b807: core 2 is the cause.** Directed requests at 6 Mbit/s, acknowledged of 3:

| Network (channel, signal) | all cores | table 2 given b43's gains | core 0 off | core 1 off | core 2 off |
|---|---|---|---|---|---|
| `Xiaomi_7C34` (1, −43) | 0 | 0 | 0 | 0 | **3** |
| `un2` (1, −43) | 1 | 1 | 0 | 1 | **3** (6 tries) |
| `CoolNet2` (8, −43) | 3 (18 tries) | 3 (10) | 0 | 2 | **3** (3 tries) |
| `Xiaomi_7C34` (6, −44) | 0 | 0 | 0 | 0 | **3** |

- **Core 2 off: the access points' OFDM acknowledgements are received, nearly at the first try.** With core 2 on, OFDM is received only from strong senders.
- **Per-core powers of the good OFDM frames:**
  - core 0 about −44 dBm;
  - core 1 about −72 to −85 dBm, 30 dB weaker (its antenna, or its gain);
  - core 2 about −45 dBm when on.
- **Table 2 given b43's gains changed nothing.** It moved core 0's reported power to about −61 dBm, so table 2 is read for core 0's report [inference]. The change is dropped.
- **Core 2's front end is now off in the radio by default.** Its transmitter is not used (frames go out on cores 0 and 1).
- **Why core 2 spoils OFDM is open:** its RX IQ, phase or gain, with no receive calibration in b43's path.
- **`TP-LINK_POCKET` (channel 6) and `David_2.4G` (channel 1) acknowledged nothing in this run,** even at 1 Mbit/s; the other networks on channel 1 did. They were the first two probed.
- **The next run** sends 1 Mbit/s and OFDM at 6, 12, 24 and 54 Mbit/s with core 2 off, then 6 Mbit/s with core 0 alone, and 6 and 1 Mbit/s with every core on.

## Related

[550-DRV-0026](../issues-done/550-DRV-0026-bcm4331-transmits.done), [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md), `bcm_wifi/`.
