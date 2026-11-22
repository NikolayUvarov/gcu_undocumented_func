# DRV landing request: `rtw89_wifi`: port 0 set for INFRA at the link (213-DRV-0091 step 2), and the day's instruments and bench switches committed off (213-DRV-0091, 0102, 0104)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-22 19:55 UTC · **Concerns:** 213-DRV-0091 (step 2 and its instruments; stays open), 213-DRV-0102 (the detection floor; stays open), 213-DRV-0104 (the send's answer; stays open for NET's answer)

**Ready:** `claude/213-DRV-0102-floor-and-answer` f4ed1a2c6

It holds today's `main` (7346ab3d5, merged at afb343dfc, no conflict but `issues/README.md`'s rows, both kept). It was tested at 4abae4eaa; the one commit after it, f4ed1a2c6, is text only (213-DRV-0091's and 0104's issues).

## What it carries

No ABI change, no IDL change. `rtw89_wifi/src/`: `active_control.rs`, `data_mode.rs`, `detection_floor.rs` (new), `main.rs`, `ppdu.rs`, `receive_service.rs`, `rx.rs`, `service.rs`, `station.rs`, `station_update.rs`, `tx.rs`; `tests/rtw89_service_cases.rs`, `rtw89_tx_cases.rs`, `rtw89_data_mode_device.rs`, `rtw89_ppdu_host.rs`, `rtw89_rx_host.rs`; the three issues and their two new rows. `docs/profile`: no statement there changes (the `rtw89_wifi` row's capabilities are as they were).

- **Step 2 of 0091, `service::PORT_AS_LINUX` true.** One beacon interval and a millisecond after the firmware is told of the link (step 1, in `main`), with no send pending, port 0 is set as Linux's `rtw89_mac_port_update` sets it for INFRA: the function off, the TSF and the beacon count reset, NET_TYPE INFRA, RX_BSSID_FIT_EN, TSF_UDT_EN, TBTT_PROHIB_EN and BRK_SETUP, the function on. The station's register check then expects PORT_CFG & 0x13E1F = 0x1281C. A deauthentication, a disassociation or a new filter sets NO_LINK again.
- **Bench switches, committed so that the driver does as `main`:** `detection_floor::DETECTION_FLOOR_AS_LINUX` false and `DETECTION_FLOOR_ALTERNATES` false (0102); `service::LATE_ANSWER` `NotAcknowledged` (the late answer as `main`'s), `SEND_BEHIND_LATE` false, `SEND_BEHIND_LATE_ALTERNATES` false, `POLL_TIGHT_WHILE_PENDING` false, `POLL_TIGHT_ALTERNATES` false (0104); `receive_service::MINUTE_DETAIL` false (below). The older ones are as in `main`: `HALT_INTERRUPT_AS_LINUX` false; `KEEP_LATE`, `KEEP_SENDING`, `OUTLAST_SER`, `OWN_DEADLINES`, `RESTART_AFTER_FAILURE`, `REPORT_EACH`, `RPQ_ALONE`, `DROP_UNACKED` true.
- **The instruments.** Their counts are kept in every build (the RX rings' polls and gaps, the steps' waits and work, the sends by outcome, size, rate and tries, the frames received by rate, the PPDU reports by signal, the submission's fetch and release). Their 27 lines a minute, and the held-frame probes, print only with `MINUTE_DETAIL` true, which the bench raises.

## What a machine that boots it does differently from `main`

- **Port 0 for INFRA** at each link, and the check expects it.
- **The log.** Every minute: `main`'s three lines (TRAFFIC, THE BEACONS ABOVE, THE FIRMWARE'S ERROR CODES), nothing more. Once a link: `PORT 0 SET FOR INFRA ON CHANNEL c (…; 213-DRV-0091)`, with the registers after it in five pieces (`PORT AFTER 1` to `5`) for the first two links; at each admission four `CALIBRATION ON c PATH p: …` lines (IQK and DPK by path, PASS or FAIL by Linux's criteria); `SER_DBG_INFO STEP n: …` at each step of SER_DBG_INFO a check sees (boot 96's log: one). Boots 93, 96 and 99, counted by script: 10 such lines at the link, against 30 lines a minute (about 4.7 KB) that the detail printed. With it off, a day's log is `main`'s.
- **The work,** with the switches as committed:
  - SER_DBG_INFO read at every liveness check (each full poll, every CHECK_MS of 10 ms; each send's begin; twice a step while a send is pending), where `main` reads it only on a halt: one register read more each;
  - four BB registers read once a minute (the detection floor's, for the minute's counts);
  - a clock read (one system call) at each RX ring poll, three more a step for the step's wait and work, one at each submission and one at each look for a pending send's release;
  - counting per frame and per PPDU report (array increments).
  - No register is read for the held-frame probes or the beacon phase: those wait for `MINUTE_DETAIL`.

## Tested

- **On LEG9, on 4abae4eaa:**
  - `02_build.sh`: success;
  - the full `scripts/host_tests.sh` with `MIND_SSHD`: exit 0; 265 Rust result lines, 6687 passed, 0 failed, 22 ignored (counted from its log by script);
  - the 40 `tests/rtw89_*_host.rs` files: 4890 cases, 0 failed. New since the last request for this driver, among them: SER_DBG_INFO's steps placed at their check; the RX rings' polls and gaps; the held-frame probe; the calibrations judged by Linux's criteria; the floor's arithmetic at −60 dBm and at its clamps; every field written and read back, those not read back named; the floor alternating by the minute; a PPDU report with no RPL binned by its header; a frame parked behind a late one, released by its sequence in either order, one at a time, never MGQ; the sends' and the steps' minute counts; the tight poll and its alternation; the port set after a beacon interval and NO_LINK again; the detail switch both ways.
- QEMU has no RTL8852BE; no QEMU suite reaches this code.

## On the Ryzen (RTL8852BE, channel 8, the access point at about −60 dBm; MC-12.1)

| Boot | Release | What ran beside step 2 | What the log shows |
|---|---|---|---|
| 80 to 99 | the bench's | `PORT_AS_LINUX` true in every one | `PORT 0 SET FOR INFRA` about 100 ms after the link (boot 99: 12.658 and 12.761 s); PORT_CFG read 1E81C in every probe after it (boot 99: 27 of 27), C004 in the two before. No stop of the data mode in the logs of boots 93, 95, 96, 98 and 99 (those read for this request). No rate or late-frame difference shown against the port as before |
| 93 | 202611221429 | the floor alternating (the OFDM floor alone in force) | late frames per send −0.004 [−0.018, +0.009]; holds, first tries the same within the intervals; 400 fewer other stations' PPDUs a minute |
| 95 | 202611221605 | the floor alternating, Linux's values with EDCCA 66 | no effect on uploads, late frames or holds within the intervals |
| 96 | 202611221648 | SEND_BEHIND_LATE alternating | busy refusals 135 → 19 of about 6400 sends; uploads unchanged (p 0.70); 12 of 27 parked frames not acknowledged |
| 98 | 202611221746 | the rate lines | our sends at 36 and 48 Mb/s, 1.3 to 1.6 tries; the AP's at legacy OFDM only; an in-time answer 4 to 6 ms |
| 99 | 202611221811 | the tight poll alternating | in-time answers within 2 ms: small frames 0 → 56%, full ones 0 → 27%; uploads 3.5 s OFF against 4.6 s ON (p 0.48); about 0.8% of a CPU; the 2 ms wait back after 5 to 20 ms about 370 times a minute, idle, pinging or uploading, and never in the cameras' whole minutes |
| 100 | 202611221915 | **this head's `rtw89_wifi` exactly, no switch raised** (the bench tree's other parts beside it) | the boot's whole log, 0.5 to 1686 s: `PORT 0 SET FOR INFRA ON CHANNEL 8` at 6.515 s; 28 minutes of `main`'s three lines and no detail line; 17,468 sent, 17,425 acknowledged, 27 late; no stop, no stall, no SER step, 0 holes; three `netbench` runs over the LAN and a fourth that stopped in `netstack` (not the radio: `NET/2026-11-22T1955Z-DRV-…`): download 196 to 367 KiB/s, upload 61 to 63 KiB/s |

- **Not shown on hardware:** the committed build has one boot of 28 minutes (boot 100), no more; every boot before it printed the detail lines, and boots 93, 95, 96 and 99 raised a bench switch. Nothing on another machine, access point or band (MC-12.9); 5 GHz not at all. Tests are not proofs (MC-12.2).
- Boot 99 had one start failure, not this branch's path: the first driver's receive-only stage read DMA_STOP past its budget at 5.6 s, stopped cleanly and was restarted by `init`; the second came up and joined.

It is in `fast-test` (55d91488b).
