# 213-DRV-0104 — `rtw89_wifi`: the send's answer, the driver's side

**Type:** driver (`rtw89_wifi`) · **Owner:** `DRV` · **Priority:** P1 · **Status:** open (2026-11-22) · **Blocked by:** NET's answer to DRV's message of 2026-11-22 14:40 UTC (steps 2 and 3 below) · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.3, MC-12.4

## Problem

On a busy channel the RTL8852BE holds a frame in the MAC after its DMA fetched it. Boot 93 held 8% of its sends, for a median of 160 ms. The frame then goes out and is acknowledged, at the first try in about 69% of them (213-DRV-0091, 213-DRV-0102).

The driver and `wlan` turn each hold into losses. Main at 1e9d8beb5 for `wlan` and `dot11`; this branch for the driver:

1. **One send at a time.** A send while one is pending is answered `busy` at once (rtw89_wifi/src/service.rs:612). Under it, `tx::Queue` keeps one request (tx.rs:180) and one submitted descriptor (tx.rs:189).
2. **The late answer.** At COMMAND_US less FRAME_MARGIN_MS, 90 ms (service.rs:613), KEEP_LATE answers `sent { acked: false, attempts }` with the tries seen so far (service.rs:280). It then awaits the release LATE_MS (5 s) more (service.rs:277). Until that release every send is refused as in 1.
3. **`wlan`** records an answer `station.sent(sent.acked, sent.attempts)` (wlan/src/main.rs:469). It records any error, `busy` included, as `station.sent(false, ATTEMPTS)`, a frame failed 15 times (:470). The frame is not offered again.
4. **`dot11::Station::sent`** (dot11/src/station.rs:443) steps the rate down for each frame not acknowledged.

So one hold of 200 ms makes:
- a false loss and a step down for the held frame, which then arrives;
- for every frame offered meanwhile, a drop, a 15-try failure and another step down.

`wlanhw.wit` 2.0 itself says `send` "answers when the hardware has its status". The 90 ms answer is the driver's own bound (COMMAND_US). `wlan`'s call deadline is RADIO_MS, 2000 ms (wlan/src/main.rs:44, :888).

DRV asked NET for three steps, cheapest first:
1. `busy` is not a failed frame;
2. a `pending` outcome;
3. an asynchronous window.

This issue is the driver's side of each, and what the driver can mend alone now.

**The measure this task is judged by: the upload rate through the Effector server.** Boot 93 (2026-11-22), from the agent's own lines, from the gateway's `open-file … admitted` to the agent's `COMPLETED` or `FAILED`:
- one read failed after 14.3 s (`POST /api/commands/file: Transport`);
- the next took 424,855 bytes in 13.8 s, about 31 KB/s;
- an earlier one took 77,671 bytes in 2.2 s, about 35 KB/s.

Linux does 480 to 826 KB/s on the same channel in the same room (213-DRV-0102).

## What Linux does (pinned source at 05f7e89)

- **Many frames in flight.**
  - Each frame takes a TX WD page from a pool of up to RTW89_PCI_TXWD_NUM_MAX (512, pci.h:1107) and a buffer descriptor of its channel's ring (RTW89_PCI_TXBD_NUM_MAX 256, pci.h:1105).
  - The WD's TXWP info carries the page's sequence (`txwp_info->seq0 = txwd->seq | RTW89_PCI_TXWP_VALID`, pci.c:1518).
  - `rtw89_core_txq_schedule` (core.c:3915) pushes as many frames from mac80211's TXQ as the ring and the WD pool have room for: `rtw89_check_and_reclaim_tx_resource`, the smaller of the free BDs and free WD pages (pci.c:1256 to 1308). It then kicks the DMA once.
  - Nothing is refused: frames beyond that room wait in mac80211's queues.
- **Release by sequence.**
  - Each RPP on the release queue carries its WD's sequence, QSEL and status (RTW89_PCI_RPP_SEQ [30:16], TX_STATUS [15:13], QSEL [12:8], MACID [7:0]; pci.h:1488 to 1491).
  - `rtw89_pci_release_rpp` (pci.c:596) takes `pages[rpp_info.seq]` directly. So releases may come in any order across channels, and each finds its own frame.
- **The outcome.** `rtw89_pci_tx_status` (pci.c:460) gives mac80211 ACK for RTW89_TX_DONE and no ACK for RETRY_LIMIT, LIFE_TIME and MACID_DROP (core.h:3528 to 3531). It gives no try count: the PCIe path has none.
- **So a hold in Linux delays the frames behind it in the same queue; it drops none and reports none as lost.**

## What this hardware and this driver already give

- **The rings.** Five TX channels of 32 descriptors each (tx.rs:8). `submit` keeps one free (tx.rs:193), as Linux's `rtw89_pci_get_avail_txbd_num` does (pci.c:1212).
  - ACH0 to ACH3 carry the data frames by TID (txdesc.rs:38).
  - The management queue (MGQ, channel 8 in the WD) carries management frames.
- **The buffers.** 32 of 4096 bytes, taken in turn by `serial % 32` (tx.rs:195). So at most 31 frames can be in flight before a buffer still in use is written again.
- **Each frame names itself.**
  - Its WD carries the WP sequence `serial & 0x7fff` (tx.rs:194; txdesc.rs:65, 66).
  - The RPQ's report gives sequence, status (Done, RetryLimit, Lifetime, MacIdDrop), QSEL and MAC ID (txdesc.rs:78 to 83).
  - `tx::Queue::reports` already refuses a report that is not its frame's (tx.rs:218).
- **Tries.**
  - Data frames go with a hardware limit of one try (SELECTED_LIMIT, txdesc.rs:7). The driver resubmits them, with the retry bit, up to SEND_TRIES 8 times (service.rs:23; tx.rs:244 to 250). It counts one try a submission.
  - The RPQ has no count (txdesc.rs:85).
  - MGQ frames wait for the firmware's final report (RPQ_ALONE, tx.rs:49 to 54, 97), which has the count. That report names its frame only by the sequence's low 4 bits (txdesc.rs:94).
- **Order.** Within one channel the MAC sends in ring order. A frame put on the ring behind a held one goes out right after it, with no round trip through the host.

## The driver's side, step by step

**Step 1 (NET's alone).** Nothing changes in the driver. Its `busy` is already answered at once, and is true: nothing of that frame went to the air. Once 1 is in, a hold no longer drops frames or steps the rate down for them. They are offered again, behind the held one.

**Step 2: a `pending` outcome** (a minor of `wlanhw.wit`, NET's to version, MC-12.4).
- **The driver's change** is one place: the late answer in `Service::progress` (service.rs:277 to 283), today `Sent { acked: false, attempts }`.
  - This branch puts it behind one constant, `service::LATE_ANSWER`, committed `LateAnswer::NotAcknowledged` (the driver as it was).
  - The day `sent` has the outcome, that constant gets `LateAnswer::Pending`, which answers `pending` with the tries so far. The frame stays pending in the driver as now, and its release is logged as now (LATE FRAME WAS RELEASED).
  - The exact text depends on the shape NET chooses, a third value or a flag. It is written then: the bindings in libmind/src/idl/wlanhw.rs are generated (`scripts/mind_idl.py`), and the driver does not invent a field the interface does not have.
- **What stays as it is:** the 90 ms, the refusal of every other send until the release, and `wlan`'s 15-try failure for each refusal (until NET's step 1).

**Step 3: the asynchronous window** (a new major of `wlanhw.wit`, NET's design; the radio's side is DRV's).
- **What the driver would keep:**
  - A window of N frames, N at most 31, the buffers' bound above. Each frame holds its buffer, its channel, its descriptor's index, its WP sequence and its tries so far, in a table indexed by `sequence % 32`. That is the same index as its buffer, so a frame and its buffer are one slot.
  - `send` answers at once: taken (its sequence, or a token the interface names), or `busy` when the window or its channel's ring is full.
- **Releases.**
  - An RPQ report finds its slot by sequence, and checks QSEL and MAC ID against the slot as `reports` does now. A report for no slot is refused as now (Error::Report).
  - DMA completion stays per channel and in ring order (`completed[q]`, tx.rs:168 to 176). A slot is final when its descriptor is completed and its report has come.
  - Releases can come out of order across channels. Within one channel they follow the ring.
- **Outcomes.** Each final frame gives acknowledged or not, its tries (submissions times SELECTED_LIMIT), the rate it was sent at and how long it was held. They come as an event or with the next answers, as NET chooses. With the RPQ alone there is no hardware try count; the count is ours, and says so.
- **Resubmission.** A data frame not acknowledged is resubmitted by the driver today. In a window, a resubmission goes behind younger frames of its TID, which reorders them (no block-ack session, so the access point passes them up in that order). Two ways out:
  - the hardware's own retries: a TX count limit in the WD instead of SELECTED_LIMIT 1, for data queues only, compared on the bench first;
  - or no resubmission once a younger frame of the same channel is on the ring, the outcome then final.
  This is to be decided with NET: it is the rate choice's input.
- **MGQ.** Its frames keep one in flight: their firmware report names a frame only by 4 bits of sequence, and the window of that queue stays 1.
- **A recovery or a stop.**
  - An L0 recovery the data mode outlasts extends every slot's deadline, as OUTLAST_SER extends the one frame's now.
  - The firmware's MACID drop or a lifetime end releases a slot as not acknowledged (DROP_UNACKED, tx.rs:56 to 58).
  - A stop answers every slot `failed` and takes nothing more. The rings are stopped as now (tx.rs:324).
  - A slot not released within LATE_MS still stops the service (TransmitTimeout), as now.
- **What the driver needs from the interface:**
  - an answer that means "taken";
  - a way to give each frame's outcome later, naming the frame;
  - `busy` meaning "the window is full";
  - a way for the station to learn the window's size, or a fixed one.
- **The service loop's cost:**
  - Each step already polls the RPQ and reads every channel's index (`reclaim`). A window adds a table lookup a report, and a scan of at most 31 slots for the deadlines once a CHECK_MS.
  - The per-step H2C and register work that waits for "no send pending" today (the link, the port, the floor: service.rs:359, 381, 402) would wait for an empty window, or be made independent of it. To be measured.

## What the driver can mend alone now

**A frame behind a late one** (`service::SEND_BEHIND_LATE`, a bench switch, committed false).
- **Today:** once a send was answered late (its call closed, its release awaited), every send is refused `busy` until that release (median 160 ms, up to seconds). Each refusal is a dropped frame and a 15-try failure in `wlan`.
- **With the switch:**
  - The late frame is parked in `tx::Queue`: its descriptor stays on the ring and its release is awaited by its sequence. It is not resubmitted: a resubmission would go behind the next frame.
  - The next send is taken and put on the ring behind it, its call open as any other. It is answered within its own 90 ms with its true outcome if the parked frame clears in time; otherwise it is answered late in turn, as today's late frame.
  - A frame is never refused because one ahead of it is held. `busy` comes only while one is parked and the next is late too: one parked frame at a time.
- **The price:** a parked frame's resubmissions.
  - Wlan was already told it was not acknowledged, at 90 ms.
  - Of boot 93's late frames, 69% went at the first try and 1% never (213-DRV-0102). The rest needed resubmissions, which a parked frame no longer gets: those reach the access point only through TCP.
  - Whether that beats today's drops is for the bench to show: late frames per send, refusals a minute, and the upload rate.
- **MGQ frames are not parked:** they wait for the firmware's report.
- The link, port and floor changes wait for no frame pending or parked, as they wait for no frame pending today.
- **Written behind the switch, committed false; not run on the Ryzen (MC-12.3).** Its progress is below.

## Plan

1. This note, for NET with DRV's message.
2. `LATE_ANSWER` (step 2's one place), committed as the driver is.
3. `SEND_BEHIND_LATE` with host cases (the Queue: park, release by sequence in either order, the buffer bound, MGQ not parked; the service: the frame behind a late one taken and answered at its release, `busy` when the next is late too), the rtw89 host files and the release build.
4. On the bench: paired minutes again, one minute with the switch and one without, if the owner takes it.

## Acceptance criteria

- [ ] NET's answer on steps 1 to 3, and this note revised to it.
- [x] LATE_ANSWER in, the driver unchanged with it committed.
- [x] SEND_BEHIND_LATE in with its host cases; the rtw89 host files and the release build pass.
- [ ] If run on the bench: compared by paired minutes on the Ryzen (MC-12.1), with what it shows and does not.

## Progress

**2026-11-22, LATE_ANSWER** (fd4c0fb87):
- The late answer is `late_answer()` behind `service::LATE_ANSWER`, committed `NotAcknowledged`. The driver is unchanged.

**2026-11-22, SEND_BEHIND_LATE, written** (this branch; host cases, not run on the Ryzen):
- **`tx::Queue`:**
  - `park()` moves the pending frame to `parked`. It refuses (State) a frame whose firmware report is awaited (MGQ, or the strict path), and refuses (Busy) a second park.
  - `reports` gives a release report to the parked frame when its sequence is the parked one's, checking QSEL and MAC ID. The rest go to the pending frame as before.
  - `ingest` takes reports while only a parked frame is out.
  - `parked_release()` finds it final once its descriptor is fetched and its release reported:
    - acknowledged when Done and not to a group;
    - one try more for Done or RetryLimit, none for a drop (Lifetime, MacIdDrop), as `progress` counts them;
    - never resubmitted.
  - `begin` refuses (Busy, without poisoning the queue) a frame whose submissions would come round to the parked frame's buffer. `submit` keeps the same bound.
- **`DataSession`:** `park()` (the data mode's TX deadline cleared with it), and `parked_release()` under the same live, reconcile and OUTLAST_SER prologue as `advance`.
- **`Service`:**
  - With `SEND_BEHIND_LATE`, a send arriving while the pending one was answered late parks that one and begins behind it. Otherwise, as before, it is answered `busy`.
  - Each step settles the parked frame's release after the pending one's, by `finish_parked`. With a send pending, `advance` has made the checks and the reconcile in that step, so the parked release looks only for a recovery (OUTLAST_SER). With none pending it makes the full prologue, as `advance` would. So a parked frame adds two register reads a step and the queue's own index reads, not a second set of checks. A parked frame not released by its deadline (LATE_MS) stops the service as TransmitTimeout, as a late one does.
  - A parked release is counted as a late frame's in the minute (late, first try, never, hold, phase) and logged as LATE FRAME WAS RELEASED. That accounting is shared with `finish_send` (`late_released`, `release_phase`).
  - The link, the port, the floor and a new filter's unlink wait for no frame pending or parked.
- **Host cases:**
  - `a_parked_frame_is_released_by_its_sequence_in_either_order_and_never_resubmitted` (tests/rtw89_tx_cases.rs): releases in both orders, and a parked RetryLimit and MacIdDrop final with no second submission.
  - `one_frame_is_parked_at_a_time_management_never_and_its_buffer_is_kept`: MGQ refused and not poisoned; one park at a time; 29 frames behind a parked one; a frame of two submissions refused busy at the bound, and one of one taken; at the next the bound refuses; the parked release, read with no send pending, frees it.
  - `service_sends_behind_a_late_frame_when_the_switch_is_on` (tests/rtw89_service_cases.rs), over the model's RPQ releases held back (`hold_rpq`) with no firmware report (fault 7):
    - without the switch the second send is refused busy;
    - with it the second is taken behind the late one and answered `acked` at its release, and the late one's release is counted;
    - a third is refused while one is parked and the next is late too;
    - four frames went out four times, and sends go on.
  - The 40 `rtw89_*_host.rs` files pass: 4886 cases, 0 failed. The release build builds.
- **Not shown:**
  - Nothing of it ran on the Ryzen.
  - The hardware's order of releases for two frames of one channel, one held, is taken from the ring's order and Linux's handling, not measured.
  - Whether it beats today's refusals is the bench's question.

**2026-11-22, the minute's sends and SEND_BEHIND_LATE_ALTERNATES** (this branch; host cases, not run):
- **A new minute line:**

  ```
  [RTW89] TRAFFIC ON c: SENDS a ANSWERED LATE, b REFUSED BUSY, p PARKED; PARKED RELEASED r, q ACKNOWLEDGED; THIS MINUTE BEHIND LATE ON|OFF (213-DRV-0104)
  ```

  - It counts the sends answered at their deadline, those answered `busy` (the refusal behind a late one, a deferral that failed, or a begin the queue refused busy), the parks, and the parked frames released and acknowledged.
  - Without the alternation the last field reads `BEHIND LATE ALWAYS` or `BEHIND LATE NEVER`.
- **`service::SEND_BEHIND_LATE_ALTERNATES`, a bench mode, committed false:**
  - With it true, SEND_BEHIND_LATE follows the minute line's minutes: the partial first minute as committed (never), then off, then on, turn and turn about.
  - A switch is made at the first poll after the minute's line with no send pending or parked, so a frame held across the boundary keeps the old kind until its release.
  - Each minute's line names what it held.
- **Host cases:**
  - `service_alternates_sending_behind_a_late_frame_by_the_minute`: the partial minute never; off refuses the send behind a late one; on parks; a frame made late two seconds before the boundary keeps "on" until its release; then off. Each line's counts are checked.
  - `service_probes_held_frames_and_counts_how_the_minutes_sends_ended` now also counts a refusal and ten late answers.
  - The 40 `rtw89_*_host.rs` files pass: 4887 cases, 0 failed. The release build builds.

**2026-11-22, boot 96 ("boot B"): SEND_BEHIND_LATE alternating on the Ryzen, under the uploads** (the bench tree 9b34fca4f with this branch to ae508f493; the floor as found in every minute; 16:53 to 17:24 UTC):
- **The load and the log:**
  - The load was the same as boot 95's: 225,391 bytes every 37 s for 30 minutes.
  - The cable's ring was read at 16:52, 16:55, 17:02, 17:12 and 17:22, and joined with the owner's tail: 31 minute lines (15 OFF, 15 ON, 14 triples) and 47 uploads, 5 of them straddling a boundary.
  - No stop, timeout, recovery or fault line.
- **Parking runs on the hardware.** In the ON minutes 27 frames were parked, all 27 released, 15 of them acknowledged.
- **The figures.** ON less the mean of its two OFF neighbours, from the minute lines by `paired.py --kind behind`:

  | Figure | OFF | ON | Paired |
  |---|---|---|---|
  | Busy refusals | 135 of 6406 sends | 19 of 6359 | −0.019 a send offered, lower in 12 of 14 (sign test p 0.013) |
  | Late frames never acknowledged | 1 | 13 | — |
  | Frames lost to `wlan` (refused + never) | 136 (2.1%) | 32 (0.5%) | — |
  | Late frames per send | 0.014 | 0.015 | +0.002 [−0.002, +0.006] |
  | Late frames' holds | median 148 ms | 144 ms | −6 ms [−34, +22] |
  | Uploads, machine-timed | 21, median 8.1 s, mean 10.2 s | 20, median 7.7 s, mean 8.9 s | rank-sum p 0.70; SD about 4.9 s, about 4.3 s resolvable |

- **Shown:** parking turns most refusals into frames sent. It loses 12 of 27 parked frames that a resubmission would probably have saved.
- **Not shown:** any change in the upload time. Boot 95's uploads (the floor as found in its OFF minutes too) took a median 4.9 s, and boot 96's 8.1 s, so one hour against the next differs more than either switch.
- **So a 225 KB upload is a poor probe of the send path.** With 98.5% of sends in time and under 2.5% lost either way, it takes 5 to 8 s. That is about 32 ms a full frame, one frame at a time, and also four or five rounds of TCP's slow start over a 300 to 900 ms round trip to the server. These logs cannot tell the two apart: the driver did not log in-time send times, and netstack logs no round trip or window.
- **A resubmission-preserving variant would need:**
  - The parked frame keeps its `Request` (its bytes and tries), not only its submission.
  - On a release not acknowledged, below its limit, the parked frame is submitted again, with the retry bit, on a submission slot of its own beside the pending one's. That is a second `Pending`, a buffer counted in the bound, and its WP sequence matched as now.
  - Its new try goes behind the frame behind it: one frame of reordering in the TID. TCP's three-duplicate threshold is not reached at a window of two.
  - The other way is the hardware's own retries (the WD's TX count limit above SELECTED_LIMIT 1, for data queues only): no reordering, but the RPQ gives no try count. That is NET's to accept (`attempts` unknown).

**2026-11-22, each send's time to its answer** (this branch; host cases, not run):
- **Why:** an upload of 225 KB takes 5 to 8 s whether frames are parked or refused (boots 95 and 96). That is about 32 ms a full frame, sent one at a time. So what caps it may be the time a frame takes when it is in time, or the time before the next one comes. The driver did not log either.
- **Four lines a minute:**

  ```
  [RTW89] TRAFFIC ON c: SENDS ANSWERED IN TIME BY MS, 0 TO 30 IN 2: [...] (213-DRV-0104)
  [RTW89] TRAFFIC ON c: SENDS ANSWERED IN TIME BY MS, 30 TO 90 IN 10: [...]; PEAK n SENDS IN A SECOND (213-DRV-0104)
  [RTW89] TRAFFIC ON c: FROM AN ANSWER TO THE NEXT SEND BY MS, 0 TO 30 IN 2: [...] (213-DRV-0104)
  [RTW89] TRAFFIC ON c: FROM AN ANSWER TO THE NEXT SEND BY MS, 30 TO 90 IN 10: [...], 90 OR MORE n (213-DRV-0104)
  ```

  - The first two lines bin each send answered in time by the time from its arrival to its answer, in the service's millisecond clock.
  - The last two bin the time from the last answer (or refusal) to the next send's arrival: wlan's and netstack's turn.
  - The bins are 2 ms to 30 ms, then 10 ms to 90. At 2 ms throughout to 90 they would take six lines a minute.
  - The peak is the most sends that arrived within one second, by seconds that begin at an arrival.
- **Host case:** `service_probes_held_frames_and_counts_how_the_minutes_sends_ended` now checks two sends in time binned under 10 ms, 12 gaps for 13 arrivals, and the peak.
- The 40 `rtw89_*_host.rs` files pass: 4888 cases. The release build builds.

**2026-11-22, the rates** (this branch; host cases, not run):
- **Why:** the owner's pings from the link's shell on boot 96 (17:34 to 17:37 UTC) to the gateway, 300 each:
  - 56 data bytes: median 4.8 to 7.8 ms;
  - 1472 bytes: median 18.1 to 20.8 ms, least 3.8 ms;
  - no loss to speak of.
  - So a full frame can go fast, and mostly does not. At 10 to 15 ms a full frame, one at a time, the uplink cannot pass 100 to 150 KB/s. The rates in use and the tries, ours or the access point's, are the next thing to see.
- **The minute lines:**
  - The in-time answers of 1dcdbb5e5 are now split by the frame's size (under 200 bytes, 200 to 1000, over 1000), one line each, the bins apart by spaces. The gaps to the next send and the peak a second share one line.
  - New lines:

    ```
    [RTW89] TRAFFIC ON c: SENT TO ONE STATION BY RATE, SENDS/ACKED/TRIES, 1 TO 9 MB/S: 6M n/a/t … (213-DRV-0104)
    [RTW89] TRAFFIC ON c: SENT TO ONE STATION BY RATE, SENDS/ACKED/TRIES, 12 TO 54 MB/S: 24M n/a/t … (213-DRV-0104)
    [RTW89] TRAFFIC ON c: SENT TO ONE STATION BY TRIES, 1 TO 15: … (213-DRV-0104)
    [RTW89] TRAFFIC ON c: RECEIVED FOR US n, r WITH THE RETRY BIT; BY LEGACY RATE: … (213-DRV-0104)
    [RTW89] TRAFFIC ON c: RECEIVED FOR US BY HT RATE: MCS7 n …; OTHER o (213-DRV-0104)
    ```

  - A unicast send is counted when it ends (in time, late or parked): the rate `dot11` asked for in the call, which the descriptor carries as a fixed rate; acknowledged or not; and the tries. That is one hardware try a submission (SELECTED_LIMIT): the RPQ gives no count and no final rate.
  - A frame for this station (addressed to its own MAC) is counted as it is taken for the station, by the data rate in its RX descriptor (word 1, bits 16 to 24, `rtw89_hw_rate`: CCK, OFDM, HT MCS 0 to 15, other) and by the retry bit of its frame control.
- **What `dot11`'s rate choice does with our answers** (dot11/src/station.rs; main at 1e9d8beb5):
  - `sent` (:443) charges the rate the frame went at with its tries, and an acknowledgement or none.
  - A frame not acknowledged at the current rate steps the rate down at once (:449).
  - Every RATE_WINDOW (20) frames (:107), each rate with at least MIN_TRIES (4) tries this window (:111) averages its success per try half and half with its past (:453 to 458). The rate with the least airtime is then chosen (`airtime_us`, :118: 24000/rate + 200 µs + 1000 µs for each failed try's share), among those that succeed at least 13/256 of the time, if it saves a sixteenth (:460 to 462).
  - The faster neighbour gets one frame in 16 (`pick_rate`, :428; SAMPLE_EVERY 8, alternating). So its average needs about 64 frames, three windows, to take four new tries.
  - **A late answer** (`acked: false`, the tries so far) steps the rate down at once and charges one failed try.
  - **A busy refusal** (`station.sent(false, 15)`, wlan/src/main.rs:470) steps it down at once and charges 15 failed tries to the rate the refused frame was given.
  - Worked example, with per-try successes as on a quiet channel: 0.90 at 24 Mb/s, 0.95 at 18, 0.97 at 12. Airtime then picks 24 Mb/s (813 µs against 920).
    - One refusal in a window of 20 makes that window's 24 Mb/s success about 18 of 35, 0.51. The average falls to 0.71, its airtime rises to 1114 µs, and 18 Mb/s is chosen.
    - The next refusal does the same to 18 Mb/s. Meanwhile 24 Mb/s recovers only through its sampled tries, by half the distance every three windows.
    - So refusals at about one in 20 frames pin the rate one or two steps below what the air allows. Late answers do the same at a fifteenth of the weight.
    - Boot 96's OFF minutes refused 2.1% of sends, about 0.4 a window. Boot 82 answered 22% late.
  - **This is a prediction from the code, not a measurement (MC-12.3).** The rate lines above are what can show it: refusals differ sevenfold between SEND_BEHIND_LATE's ON and OFF minutes, so if the mechanism holds, the rates used should differ with them.
- **Host case:** `service_probes_held_frames_and_counts_how_the_minutes_sends_ended`:
  - its twelve unicast sends at 6 Mb/s, each acknowledged at one try;
  - the in-time answers by size;
  - sixteen frames for the station at the model's rate 0, one with the retry bit;
  - the bins of `rx_rate_bin`, `size_class` and `ms_bin` at their edges.
- The 40 `rtw89_*_host.rs` files pass: 4888 cases. The release build builds.

**2026-11-22, the final submission's fetch and release timed** (this branch; host cases, not run):
- **Why:** boot 98's in-time answers have their mode at 4 to 6 ms for every size, and none under 2 ms, while a frame at 36 Mb/s is on the air in under one. Two more measures split that time.
- **`tx::Queue`** keeps the pending submission's times on the bus clock: its submission, the first poll that finds its descriptor fetched, and the ingest of its release report. At a completion it keeps the final submission's two spans.
- **Six minute lines**, two a size class (under 200 bytes, 200 to 1000, over 1000):

  ```
  [RTW89] TRAFFIC ON c: SUBMITTED TO FETCHED BY US, OVER 1000 B: … (213-DRV-0104)
  [RTW89] TRAFFIC ON c: FETCHED TO RELEASED BY US, OVER 1000 B: … (213-DRV-0104)
  ```

  - The bins are 250 µs to 2 ms, 1 ms to 10 ms, then to 20 ms, to 50 ms, and over.
  - Both spans are as the polls see them, so they are quantized by the service's steps. With the tight poll below they are finer.
- **Host case:** the minute sends case checks the twelve ended sends in both and the bins' edges. All 40 `rtw89_*_host.rs` files pass. The release build builds.

**2026-11-22, boot 98's rate lines, and a tight poll while a send is pending** (this branch; host cases, not run):
- **Boot 98** (release 202611221746, bench tree 1d3f1277b with 1127b9a95; cable reads at 17:54 and 18:00; the owner's pings at 82 to 141 s, his uploads at 141 to 412 s):
  - **Our unicast sends:** mostly at 36 and 48 Mb/s, some at 24 and 54. 1.3 to 1.6 tries a frame. About 20 to 25% needed more than one try, almost all acknowledged in the end.
  - **The access point's frames to us:** legacy OFDM only, 48 and 54 Mb/s, no HT. 17 to 24% carry the retry bit.
  - **The in-time answer's time** has its mode at 4 to 6 ms for every size, nothing under 2 ms.
  - **The gap from an answer to the next send:**
    - In the ping minutes it peaks at 0 to 2 ms and at 10 to 12 ms. The second is wlan's loop: `recv_timeout(…, POLL_MS)`, POLL_MS 10 (wlan/src/main.rs:34, :905).
    - In the upload minutes it peaks at 0 to 2 ms, and again past 30 ms, TCP's turn.
  - **So in this hour a frame sent one at a time costs 4 to 6 ms of the driver's, more than its air time.** wlan's tick and TCP add their parts.
- **Why 4 to 6 ms:** a send is begun in a step's dispatch. Its release is looked for in the next step's progress, after that step's `recv_timeout` of POLL_MS 2 ms (IPC timeouts end at the first 1 ms tick after the time asked on x86, docs/api). If not released by then, one more wait follows.
- **`service::POLL_TIGHT_WHILE_PENDING`, a bench switch, committed false:**
  - After a step that leaves a send's call open, the service polls at TIGHT_PACE_US (100 µs) for up to TIGHT_US (2 ms), then goes back to its loop. Each poll:
    - reads the rings by the light poll (`poll_more`, the release queue and RXQ unless the frame buffer is full);
    - takes the station's frames and wakes its waiters;
    - settles the send by `advance_light`: the queue's progress without the data mode's liveness checks and reconcile, which the step made, but a recovery is still outlasted.
  - Requests that come meanwhile wait at most those 2 ms for the next step's receive.
  - **The pace:** there is no sleep below the scheduler's tick, and no yield in the ABI. IPC timeouts take whole milliseconds, 0 meaning without limit. So the pause is the device's `delay_us`, `io::short_delay`: a spin on the monotonic clock, at most 1 ms.
  - 100 µs keeps a light poll (about 20 MMIO reads) under a tenth of the pace. The cost is CPU: up to 2 ms of spin a send whose release is slow, on CPU 4 beside the compositor.
- **`service::POLL_TIGHT_ALTERNATES`, a bench mode, committed false:** the switch by the minute line's minutes, as SEND_BEHIND_LATE's: the partial minute never, then off, then on.
  - The SENDS line ends `THIS MINUTE TIGHT ON|OFF` (or `TIGHT ALWAYS|NEVER`) with the minute's tight polls.
  - **From outside:** let T1 be the boot's first `TRAFFIC ON c: n FRAMES` line's time (about 66 s). An uptime U, from the link shell's `time`, lies in minute k = 1 + ceil((U − T1) / 60). Minute 1 is never; even minutes are off and odd minutes from 3 are on.
- **Host case** `service_polls_tight_while_a_send_is_pending`:
  - a send answered in the step that took it with the switch, in the next without;
  - with its release held, the poll gives up after 10 to 25 polls and the send is answered at its release;
  - a frame that comes 600 register reads into the step is taken, and its waiter woken, in that step only while polling;
  - alternating, the labels and the answers' step follow never, off, on (RPQ alone, as on the Ryzen).
- The 40 `rtw89_*_host.rs` files pass: 4889 cases. The release build builds.

**2026-11-22, boot 99: the tight poll on the hardware** (bench tree a3ce183a4 with 264e3534e, POLL_TIGHT_ALTERNATES true; cable reads at 18:23 and 18:28 UTC; the owner's pings in minutes of both kinds, files S/handoff/wifi/ping/b99-*):
- **The hardware is fast.** In ON minutes the final submission was fetched within 250 µs for about 95% of sends. Small frames were released within 250 µs of the fetch, full-size ones mostly within 250 to 750 µs.
  - In OFF minutes the same spans read 2 to 4 ms to the fetch and 0 to the release: both are seen only at the next step. So the 4 to 6 ms of an OFF answer is the loop's.
- **The in-time answer:**
  - ON: under 200 bytes mostly 0 to 2 ms; full-size mostly 0 to 4 ms. A smaller bump at 6 to 10 ms is the sends whose 2 ms poll ran out, which then met the late wake-up.
  - OFF: none under 2 ms; the mode 4 to 6 ms.
  - About ten tight polls a send, 7,000 to 10,700 a minute.
- **Caveat:** the late wake-ups of the idle machine (213-DRV-0091, boot 99) make the plain loop look worse than it would be with a mended wake-up. That would give 2 to 6 ms, one or two receive waits plus the step. The tight poll spins past them.
- **Still to measure:** full-size answers take 2 to 4 ms while their release takes under 0.75 ms. The rest is before the submission: `DataSession::begin` makes the full liveness checks and the reconcile for every send, which the polls make once per CHECK_MS. Arrival-to-submission is the next span to time.
- The upload comparison of ON against OFF minutes on this boot is the owner's load, read with `paired.py --kind tight`.

**2026-11-22, the minute's detail behind one switch, for landing** (this branch; host cases):
- **Why:** boot 99's log, counted by script: `rtw89_wifi` printed 30 minute lines, about 4.7 KB, every minute; main prints 3, about 465 bytes. The log holds 2048 records of 200 bytes, so the detail alone fills it in about 68 minutes, where main's three lines take about 11 hours. A late frame added four more lines (HELD FRAME and its registers in three pieces) for the first eight of a boot, then one a minute.
- **`receive_service::MINUTE_DETAIL`, a bench switch, committed false:**
  - False prints main's three lines a minute (TRAFFIC, THE BEACONS ABOVE, THE FIRMWARE'S ERROR CODES) and the GOOD LINK probe every tenth minute, as main does. It makes no held-frame probe (HELD FRAME, HELD FRAME'S REGISTERS, IN-TIME FRAME'S REGISTERS) and reads no TSF for the beacon interval's phase.
  - True prints the 27 detail lines of boot 99 and probes as there.
  - The counting behind the lines runs either way. It costs CPU and clock reads, not register reads.
- **Host case** `service_probes_and_places_releases_only_with_the_minute_detail`: without the switch, no probe of the held frame or the in-time one and no release placed in the beacon interval; with it, both probes and the releases placed. The minute's late and tried sends are counted either way. The held-frame case raises the switch.
- The 40 `rtw89_*_host.rs` files pass: 4890 cases, 0 failed. `02_build.sh` builds.

**2026-11-22, boot 99's figures for the tight poll, and the late wake-ups under uploads** (the cable's pieces at 18:23 and 18:28 UTC and its tail at 18:42, joined: 25 minute lines to 1512.7 s; counted by `tight.py` and `paired.py --kind tight`, /home/un/.local/share/mind-bench/wifi/; their output in S/handoff/wifi/boot99/tight.txt and paired.txt):
- **What the minutes held:** 12 OFF and 12 ON after the partial first. The owner's pings fell in 7 minutes (137 to 394 s, 704 to 746 s), CAM's two cameras streamed at 465 to 650 s (two whole minutes, two at the edges), and 17 uploads of 225,391 bytes ran at 796 to 1506 s, 16 of them the owner's load from 18:31:42 UTC.
- **The in-time answer by size,** pooled, the four camera minutes left out. The share answered within 2 / 4 / 6 / 10 ms:

  | Size | OFF | ON |
  |---|---|---|
  | under 200 B | 0 / 23 / 81 / 93% of 1664 | 56 / 73 / 75 / 91% of 1678 |
  | 200 to 1000 B | 0 / 17 / 73 / 86% of 882 | 40 / 67 / 70 / 86% of 872 |
  | over 1000 B | 0 / 17 / 67 / 83% of 2998 | 27 / 61 / 63 / 86% of 2483 |

- **The final submission,** the same minutes:
  - **To its fetch seen:** ON, 92 to 95% within 250 µs. OFF, none within 750 µs, 11 to 15% within 2 ms and 96 to 97% within 4 ms: the next step's look, early when a request ended its wait.
  - **From the fetch to the release seen, ON:** under 200 B, 50% within 250 µs, 69% within 750 µs, 78% within 2 ms. Over 1000 B, 6%, 47% and 70%; 26% after 4 ms and 7% after 10 ms.
  - OFF puts 78 to 92% in the first bin: the fetch and the release are seen at the same poll, which says nothing of the hardware.
- **The record corrected.** The section above says small frames were released within 250 µs of the fetch and full-size ones mostly within 250 to 750 µs. Over the whole boot, half the small ones were and under half the full-size ones.
  - About 30% of full-size frames were not released within the tight poll's 2 ms. Their releases fall in the loop's bins again (4 to 6 ms: 336 of 2488), and 7% after 10 ms.
  - So their release came somewhere between 2 ms and the time it was seen. For a third of full frames on this channel, the 2 ms window is too short.
- **The gap from an answer to the next send:** within 2 ms, 28% OFF and 18% ON; at 10 to 12 ms (wlan's POLL_MS), 14 and 17%; at 30 ms or more, 29 and 32%. A sooner answer still meets wlan's tick and TCP's turn.
- **The uploads,** machine-timed (the gateway's `open-file … admitted` to the agent's `COMPLETED`):
  - OFF: 9, median 3.5 s, mean 4.2 s. ON: 6, median 4.6 s, mean 4.7 s. SD 1.1 s, rank-sum p 0.48. Two straddled a boundary (4.4 and 4.6 s). None failed.
  - So no effect is shown. About 1.8 s could have been resolved.
  - 225,391 bytes in 3.0 to 6.2 s is 36 to 75 KB/s, faster than boot 95's median of 4.9 s and boot 96's 8.1 s for the same file: the hour again.
  - The owner's times for the same 16 uploads, from this PC (load-boot-99.tsv), were 9.9 to 23.4 s. The machine's part was 3 to 6 s. The rest lies outside the Ryzen: the Effector server is on the Internet (hesere.uz), not on the LAN.
- **The pings,** median of each run of 200: ON 9.2 to 20.1 ms (13.3 the median of the runs), OFF 5.6 to 25.1 ms (14.7). The three OFF runs at 704 to 720 s had medians of 5.6 to 6.0 ms, and the four after them 12.6 to 15.2 ms. So whatever moves a ping's median, it is not the tight poll.
- **Late frames:** 0.004 a send in both kinds (21 of 5864 OFF, 23 of 5346 ON); busy refusals 11 and 8.
- **The driver's CPU:**
  - ON minutes made 1,623 to 10,723 tight polls (median 5,008), 11.1 a send. At their pace of 100 µs that is about 0.8% of one CPU at the median and 1.8% at the most.
  - The steps' work over 5 ms: 14 to 191 a minute in ON minutes, the longest 6.8 ms, against 0 or 1 in OFF minutes (the longest 5.1 ms).
  - The light poll's own reads are not timed. A `ps` at the start and end of an ON and an OFF minute would give the CPU time itself.
- **The late wake-ups by what the minute held** (the SERVICE STEPS line, either kind of minute):

  | Minutes | n | Waits over 5 ms a minute | Over 10 ms | The longest | Sends a minute |
  |---|---|---|---|---|---|
  | quiet | 3 | 311 to 402 (median 384) | 214 to 268 | 18.5 to 19.3 ms | 151 |
  | pings | 7 | 345 to 431 (373) | 237 to 311 | 18.0 to 20.6 ms | 949 |
  | uploads | 11 | 301 to 431 (369) | 229 to 317 | 18.3 to 19.4 ms | 404 |
  | cameras, whole minutes | 2 | 0 and 0 | 0 and 0 | 3.0 and 3.2 ms | 146 and 148 |
  | cameras, the edges | 2 | 194 and 144 | 129 and 102 | 18.0 and 18.4 ms | 155 and 147 |

  - **An upload load alone does not make the waits punctual; the cameras' streams did.** The uploads keep wlan, netstack and tls busy and the driver sending 400 frames a minute, and the waits stay as on the quiet machine.
  - What the camera minutes add that the uploads do not is not shown by these lines. usb_host answering bulk transfers from its loop (000-DRV-0101) is one candidate: a task that hardly idles.
- **A start failure, apart:** at 5.6 s the first `rtw89_wifi` stopped in the receive-only stage, `Radio(Listen(Permit(Register(Fault { kind: Timeout, register: 4112 }))))`: a read of DMA_STOP (0x1010) outran its budget. The device stopped cleanly, `init` started a fresh driver at 5.6 s, and it came up. That path (phy.rs, listen.rs) is not changed by this branch. Boots 93, 95, 96 and 98 each started the driver once.
- **Not shown:** another boot, hour, access point or machine (MC-12.1, 12.9); an upload effect smaller than about 1.8 s; the cause of the wake-ups.

**2026-11-22, the uplink over the LAN with nothing listening on the Ryzen** (reading only; nothing run):
- **Why:** every upload measured so far went through the Effector server, which is on the Internet (hesere.uz), over TLS. Boot 99's machine-timed uploads took 3 to 6 s for 225 KB, and the gap to the next send was 30 ms or more for a third of sends: TCP's turn over that path, which a LAN measurement would take out.
- **The tool exists: `netbench <a.b.c.d>:<port> [MiB]`** (issue 106, `netbench/src/main.rs`, in the image). It connects out, so nothing listens on the Ryzen:
  - DOWN: the server sends n MiB over TCP; UP: netbench sends n MiB and the server answers `OK` once it has them all; then 500 UDP round trips of 64 bytes; then the stack's checksum loop.
  - Each phase prints its rate in KiB/s and the CPU that `netstack` and the card drivers spent per MiB, from `sysmon`: `NETBENCH UP 4194304 BYTES … MS … KIB/S NETSTACK … US/MIB DRIVERS … US/MIB`.
  - **Its DRIVERS figure counts `virtio_net` alone** (`cpu()`, netbench/src/main.rs:34). On the Ryzen it reads 0; `wlan` and `rtw89_wifi` are not counted. That is a change in netbench, NET's (issue 106), not ours.
- **This PC's side:** `/home/un/.local/share/mind-bench/wifi/benchsrv.py PORT` is `tests/qemu_smoke.py`'s `_Bench` and `_bench_server` on a fixed port (TCP and a UDP echo on the same number), with a line for each connection as this end saw it. Tested on 127.0.0.1 only: 1 MiB each way and a datagram echoed. LEG9 is 192.168.2.211 on the LAN; WSL's TCP port 8724 was reachable from the Ryzen under Linux (putsrv.py). Whether UDP is reachable the same way is not known.
- **What it needs on the Ryzen,** each the owner's:
  1. Two lines of the network policy, as the QEMU suite writes them (qemu_smoke.py:16833): `netbench 192.168.2.211 tcp 8725 600 100000000` and `netbench 192.168.2.211 udp 8725`. `netpolicy add <line>` asks at the console (Y). The link's client may not change the policy; the link's console in the owner's `link-console` mode may (docs/network/policy.md:128).
  2. `netbench 192.168.2.211:8725 4` typed at the console's shell, which starts it with the flow grant. `submit run` from the cable gives a program no network grant, so it would print `NETBENCH CONNECT: NO GRANT`.
  3. Its lines go to the system log too, read over the cable as now.
- **With no typing at all,** the link's shell runs `ping`: `ping -f -s 1472 -c 500 <host>` sends the next request at its reply, one full frame at a time. That gives a frame's round trip, not a stream's rate (boot 99's runs: medians 6 to 25 ms). Whether this PC answers ICMP echo through Windows' firewall is not known; the gateway 192.168.2.1 does.
- **Not a tool to write:** netbench is the smallest that measures a stream. Only its driver count wants changing, and that is NET's.

**2026-11-22, a send's time from its receive to its submission** (branch `claude/213-DRV-0104-begin-light`; host cases, not run):
- **Why:** boot 99's ON minutes answer full-size frames in 2 to 4 ms, and the hardware fetches within 250 µs. What comes before the submission was not timed. A request is dispatched only after its step's own work (`progress`: the rings' polls and the drain, the checks every CHECK_MS, a pending send's progress, the link, the holes). `DataSession::begin` then makes the data mode's liveness checks and the coexistence reconcile for every send.
- **Two lines a minute** (with MINUTE_DETAIL):

  ```
  [RTW89] TRAFFIC ON c: RECEIVE TO DISPATCH BY US: …; FOUND WAITING n (213-DRV-0104)
  [RTW89] TRAFFIC ON c: DISPATCH TO SUBMISSION BY US: … (213-DRV-0104)
  ```

  - Each send begun is timed from its step's receive returning to its dispatch, and from the dispatch to `begin`'s return (the checks, the reconcile and the submission). The bins are those of the fetch and release lines.
  - FOUND WAITING counts the sends whose receive returned within 100 µs: the request was there before the wait began. How long it had waited, through the step before, the driver cannot see.
- **Host case:** the held-frame case's twelve sends begun are counted in both lines. The model's receive always waits its 2 ms, so none is found waiting there.
- The 40 `rtw89_*_host.rs` files pass; `02_build.sh` builds.

**2026-11-22, `BEGIN_LIGHT`: a send begun without the checks its step made** (branch `claude/213-DRV-0104-begin-light`; host cases, not run):
- **`service::BEGIN_LIGHT`, a bench switch, committed false,** and `BEGIN_LIGHT_ALTERNATES`, a bench mode committed false (the partial minute never, then off, then on, as the others):
  - With it, a send whose step made the data mode's checks within CHECK_MS (the service's `checked`, set by the full poll) is begun by `DataSession::begin_light`. That makes no liveness checks (the firmware's status, the station's registers, the radio, the permits) and no coexistence reconcile, as the polls between two checks make none. The TX queue's state and rings are checked all the same, by `tx::Queue::begin`.
  - Still made: the request's bounds, the frame's own address and descriptor, the data mode's own state (not stopped, its identity, in data mode, the session not failed: `alive`, which `live` now begins with), a recovery outlasted (OUTLAST_SER), and `tx::Queue::begin`'s own checks of its state and rings (the same that the liveness checks' `validate_data` makes).
  - A send whose step's checks are older (the queue not read empty, up to CHECK_LATEST_MS) is begun with them, as now.
  - The DISPATCH TO SUBMISSION line counts the sends begun light and names what the minute held: `THIS MINUTE LIGHT ON` or `OFF`, `LIGHT ALWAYS` or `NEVER`.
- **What it may cost:** a firmware error code, a recovery's step or a register the skipped checks would have met is met at the next step instead, by the pending send's `advance`, which makes the full checks and the reconcile before the send is answered.
- **Host case** `service_begins_a_send_light_within_check_ms_of_the_steps_checks` (data frames final on the RPQ, as on the Ryzen):
  - in the step that begins a send, the model's radio checks (its reads of 0x8040): 2 without the switch (the checks' and the reconcile's), none with it; the send answered at its release either way;
  - with the frame buffer full and 12 ms more, the step's checks are older than CHECK_MS and the send is begun with them, the same count with and without;
  - alternating, the minutes' lines: never 0, off 0, on 1, off 0 sends begun light.
- The 40 `rtw89_*_host.rs` files pass: 4891 cases, 0 failed. `02_build.sh` builds.
- **Not run on the Ryzen.** Whether it shortens the answer is the bench's: DISPATCH TO SUBMISSION in ON against OFF minutes, then the in-time answers.

## For a successor (state on 2026-11-22, 19:15 UTC)

`S` is the session's scratchpad: /tmp/claude-1000/-mnt-c-common-virtual-Culture-gcu-undocumented-func/4c527436-e074-4262-82a9-b8ea9d807c88/scratchpad. Everything before this afternoon is in [213-DRV-0091](213-DRV-0091-the-firmware-told-of-the-link.md)'s and [213-DRV-0102](213-DRV-0102-rtw89-wifi-detection-floor.md)'s sections "For a successor".

**Branches** (both pushed):
- `claude/213-DRV-0102-floor-and-answer` (worktree S/a-wifi2): main 7346ab3d5 merged at afb343dfc; MINUTE_DETAIL at 4abae4eaa; then text only (this section, boot 99's figures, the LAN uplink). The landing candidate. On 4abae4eaa, LEG9: `02_build.sh` success; the full `scripts/host_tests.sh` with MIND_SSHD exit 0, 265 Rust result lines, 6687 passed, 0 failed, 22 ignored (its log: /home/un/.local/share/mind-bench/wifi/full-4abae4eaa.log). The owner posts the landing request.
- `claude/213-DRV-0104-begin-light` (worktree S/a-wifi3): on 4abae4eaa, the receive-to-dispatch and dispatch-to-submission lines (1c4673e60) and BEGIN_LIGHT with its alternation (8e8bbbced), the landing branch's text merged in after. The 40 `rtw89_*_host.rs` files (4891 cases) and the build only; not run on the Ryzen.

**The switches as committed:** `service::PORT_AS_LINUX` true; `LATE_ANSWER` NotAcknowledged; `SEND_BEHIND_LATE`, `SEND_BEHIND_LATE_ALTERNATES`, `POLL_TIGHT_WHILE_PENDING`, `POLL_TIGHT_ALTERNATES`, `BEGIN_LIGHT`, `BEGIN_LIGHT_ALTERNATES` false; `detection_floor::DETECTION_FLOOR_AS_LINUX`, `DETECTION_FLOOR_ALTERNATES` false; `receive_service::MINUTE_DETAIL` false; `mac::HALT_INTERRUPT_AS_LINUX` false. **A bench image raises MINUTE_DETAIL,** or none of the minute lines the scripts read is printed.

**The next bench run proposed** (the owner's to take): the begin-light head with MINUTE_DETAIL, POLL_TIGHT_WHILE_PENDING and BEGIN_LIGHT_ALTERNATES true. The tight poll in every minute takes the loop's 4 to 6 ms out of the answer, so ON and OFF minutes differ in the begin alone. Read DISPATCH TO SUBMISSION and RECEIVE TO DISPATCH by kind, then the in-time answers: `tight.py LOG --kind light` labels the minutes by the LIGHT label of the DISPATCH TO SUBMISSION line.

**Tools** (/home/un/.local/share/mind-bench/wifi/, kept across restarts):
- `tight.py LOG [--cameras A-B] [--pings RUNS.tsv]...`: the tight poll's paired figures and the waits by what the minute held (boot 99's output: S/handoff/wifi/boot99/tight.txt);
- `perminute.py LOG`: the driver's lines and bytes a minute;
- `benchsrv.py PORT`: netbench's server on this PC (below);
- `paired.py`, `join_cable.py`, `boot-report.sh` as before.

**Open, in order:**
1. The landing (the owner's).
2. The BEGIN_LIGHT run above, then the in-time answers.
3. The tight window: about 30% of full-size frames were released after its 2 ms (boot 99). A longer TIGHT_US catches them at more spin; a punctual wake-up (KRN's) catches them without.
4. `advance` makes the full checks and the reconcile, then the reconcile again, every step a send is pending, and `progress` calls it twice a step. A light `advance` within CHECK_MS is the lever after BEGIN_LIGHT. Not written.
5. The LAN uplink with netbench (below), the owner's to run.
6. NET's answers on steps 1 to 3.

**Traps:**
- The full host suite compiles the worktree's files as they are: change nothing there while it runs.
- The host model's receive waits its whole 2 ms with a request queued, so FOUND WAITING is 0 in host cases.
- In host cases with the frame buffer full, a release is read only on the RPQ-alone path (`service.rpq_alone()`).
- `pkill -f` with a pattern that the shell's own command line holds kills that shell.

## Related

[213-DRV-0091](213-DRV-0091-the-firmware-told-of-the-link.md) (the held frames, the six steps), [213-DRV-0102](213-DRV-0102-rtw89-wifi-detection-floor.md) (the detection floor, boot 93's paired minutes), DRV's message to NET of 2026-11-22 14:40 UTC on `internal-dev-messages`.
