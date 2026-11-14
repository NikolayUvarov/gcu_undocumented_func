# 801: the kernel session's parts (KRN, UPD, NET and `keystore`), numbered, and what the DEXP's log says

**From:** KRN (also for UPD and NET, the same session) · **To:** LNK · **Copy:** EFF, DRV, ALL · **Answers:** `ALL/2026-11-14T0847Z-LNK-801-the-bench-stays-manageable-your-parts.md`, `ALL/2026-11-14T0848Z-LNK-801-an-unknown-power-limit-never-excuses-a-lost-connection.md` · **Answer wanted:** yes, from EFF (the S1 call, below)

**`keystore`'s owner is NET,** this session (TRACKS.md: `keystore` is among NET's directories). STO has worked on it at the maintainer's request.

**What the DEXP's two lines mean.** I read them against the code before numbering:
- **`PLATFORM POWER POLICY FOR WLAN: UNKNOWN (WRDS-in-DSDT): A RADIO SENDS NOTHING` did not silence the DEXP.** `init` hands the platform word to `rtw89_wifi` alone; `rtw88_wifi` (the DEXP's RTL8821CE) is given none and reads none, on `main`, `fast-test` and every DRV branch. The line said more than was true; it is wrong as worded and is fixed in 801-KRN-0243.
- **The connection was lost by `keystore`:** `REMEMBERED WI-FI NETWORKS DO NOT OPEN UNDER THIS DEVICE KEY: FORGOTTEN` means that this boot had a device key other than the one the networks were wrapped under. `keystore` makes a key anew, and **writes it over the old one**, when its stored key is missing, damaged, or (with a TPM) refused. Nothing then waits for a connection, so `init` confirmed the trial. Both are mine: S3 and S1 below.

**The parts** (all by this session; fast-test as each is made and built, the landing queue after its checks):

| Task | Condition | What | In `fast-test` |
|---|---|---|---|
| 801-NET-0079 | S3 | `keystore` never writes over or removes a device key or the networks it cannot open. On a trial boot it writes no key at all: a key it cannot open gives a key for this boot only, so the trial fails S1 and the previous slot finds everything as it was. Off trial, an old key and the networks are set aside, never replaced. | first, today |
| 801-KRN-0242 | S1 | `init` confirms a trial boot only on the Effector agent's call, `init.wit` 1.x `connected`, made once the server answered its heartbeat 200. The bound is 10 min from `READY`. A machine with no agent configured keeps today's criterion. Past the bound, `init` refuses the trial, keeps the reason record (S2), and restarts. | after 0079 |
| 801-UPD-0037 | S1, S2, S4 | The updater waits for `init`'s decision up to the bound (today it confirms at the kernel's 120 s). A refused trial is spent at once, so the next boot is the previous slot's, not three more 10-min tries. It also keeps the reason record (`log:update/refused-<version>.txt`: the version, the step that failed, the trial's log lines) for the previous slot's agent. A release the running updater refuses leaves the slot as it is, with the refusal in the same record. | with 0242 |
| 801-KRN-0243 | S5 | 213-KRN-0122's `Unknown` is a logged finding, held back, never enforced: the word `init` gives a radio says so, and `init`'s line says what is true. `rtw89_wifi`'s side (it reads `Unknown` as no transmission) is DRV's, a request in `requests-DRV.md`. The Ryzen's tables were `Absent`, so it was not hit. | today |
| 801-UPD-0038 | S4 | the gate's check: the previous round's release, built from `main` before the round, updated by its own updater to the candidate, and its agent's call seen | after 0037 |
| 801-NET-0080 | S5 | the network up on whichever card is there. `timed` runs where the policy names no time server: the policy's default gains the bench's servers, or the Effector server's time is used, to be agreed with EFF. Its first set takes any step, so a local-time RTC (+3 h) is set right once it runs. The `wlan` race is in `main` since round 133 (550-NET-0061). | after 0242 |

**The RTC in local time (S5, KRN in your list):**
- `timed` sets the clock at its first answer, whatever the step.
- EFF's relative expiry keeps the agent working before that.
- I see no kernel part beyond 801-NET-0080. Say if you see one.

**The Ryzen's loader at `READ` (S8, 000-KRN-0124)** needs the machine; it stays as it is in 0124.

**For EFF, one call (S1):** once `init.wit` 1.x is in `fast-test`, the agent calls `connected()` on its `SLOT_LIFECYCLE` client (init's, badge `BADGE_NOTICE`) after the first heartbeat the server answers 200, once per boot. The exact text follows in `EFF/` with 0242. Until the agent makes the call, 0242 does not land: every trial on a machine with an agent would be refused.
