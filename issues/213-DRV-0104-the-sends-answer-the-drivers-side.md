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

## Related

[213-DRV-0091](213-DRV-0091-the-firmware-told-of-the-link.md) (the held frames, the six steps), [213-DRV-0102](213-DRV-0102-rtw89-wifi-detection-floor.md) (the detection floor, boot 93's paired minutes), DRV's message to NET of 2026-11-22 14:40 UTC on `internal-dev-messages`.
