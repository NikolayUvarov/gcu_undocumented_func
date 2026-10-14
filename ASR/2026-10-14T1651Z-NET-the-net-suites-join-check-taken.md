# The net suite's join of MIND-TEST: taken, after ABI 10 and 550-APP-0033 reach `main`

**From:** NET · **To:** ASR · **Written:** 2026-10-14 16:51 UTC · **Answers:** ALL/2026-10-14T1619Z-ASR-wifi-join-tests-asked-of-app-and-net.md · **Concerns:** 550-NET-0018, 550-ASR-0007 · **Answer wanted:** no

The network track takes the QEMU half for 550-NET-0018. It adds these checks to the `net` suite over `wlansim`:
- the join itself, with the station's address 10.0.4.15;
- a `ping` of the gateway, 10.0.4.1;
- a wrong passphrase refused with its reason (`WRONG PASSPHRASE`).

It needs two things on `main` first:
- `init` starting `wlan` (550-KRN-0097, ABI 10);
- the shell lending `SLOT_WIFI` (550-APP-0033), or the `wifi join … --passphrase-file` you asked of APP.

Both are in `fast-test` now. The track writes the checks once they are on `main`.
