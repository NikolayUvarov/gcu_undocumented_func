# Ready: the station knows a fresh radio by the PID that answers it (801-NET-0085)

**From:** NET · **To:** LANDING · **Written:** 2026-11-15 19:28 UTC · **Concerns:** 801 S5, DRV (rtw89 on the Ryzen)

**Ready:** `claude/801-NET-0085-a-fresh-radio-by-the-pid` e636552783

## What it brings

- **`libmind`:** `mind::idl::wire::replied_by()`, the PID that answered the last IDL call (the kernel's reply `arg1`, now written in `docs/api`).
- **`wlan`:** a radio call answered by another PID than the one before marks a fresh radio.
  - The old station gives it nothing more.
  - At the next turn it starts anew and gives the country again, as for 801-NET-0083's other signs.
  - The line: `[WLAN] THE RADIO RESTARTED (ANSWERED BY PID <new>, NOT PID <old>)`.
- **`wlansim` built with `self-tuned`** (`scripts/x86_fixtures.sh`) starts on the access point's channel, as rtw89 took its own.
- **The `net` suite's `fresh_radio_check`** (x86):
  - it holds the station by a CPU budget (`budget`) from before the kill until the fresh radio is up, the station's sends counted unchanged;
  - then the PID alone must tell the restart, and the station joins again by itself.

## Tested

| Group | Where | Result |
|---|---|---|
| x86: network, TLS and Effector | i9 (on `7f056287dd`) | PASS 2186 s, the check's line naming the PID |
| fresh_radio_check alone, five times | x32 (on `7f056287dd`) | PASS each, 15 s, the PID telling each time |
| x86: network, TLS and Effector | x32 (on `41e360c4d4`, before the hold) | PASS 1770 s, the PID telling only at the third restart: the reason for the hold |
| aarch64: network, TLS and Effector | acc2 (on `cfd09c542a`) | PASS 1576 s |
| x86: boot, display, shell, memory, clock; x86: services, storage, audio | i9 (on `cfd09c542a`) | PASS 1761 s, 1306 s |
| host tests | x32 (on `cfd09c542a`) | PASS 2221 s |

`e636552783` adds only text.
