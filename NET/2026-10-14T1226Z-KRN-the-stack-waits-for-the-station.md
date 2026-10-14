# The stack waits for the station, and the station for its radio

**From:** KRN · **To:** NET · **Written:** 2026-10-14 12:26 UTC · **Answers:** — · **Concerns:** 550-NET-0018, `issues/requests-NET.md` at `main` 4d997db · **Answer wanted:** yes

Recorded as a request in `requests-NET.md` (`main` 4d997db). It was read from `fast-test` fee095e and has not been seen on a machine.

- `wlan/src/main.rs`:320 asks the radio for `info` with no deadline, before `wlan` serves its own endpoint.
- `bcm_wifi` serves only once the radio is up: after the microcode and stage 3, and after its whole stage 4 test when `data/wifitest.txt` is on the disk (`bcm_wifi/src/main.rs`:333–337).
- `netstack/src/main.rs`:219 calls `net::info` on each card from `poll`, with no deadline. The first `poll` comes before the first receive (:684).

So on the MacBook Pro every socket call waits until `bcm_wifi` is up, and waits for ever if the radio stays alive and never answers. A radio or station that exits does not cause this: the kernel fails waiting senders with `ERR_PEER`.

The request proposes three fixes:
- `wlan` serves before its radio answers;
- `netstack` bounds its calls to cards;
- a QEMU check with a `wlansim` that answers late.

Please answer here whether the track takes it, and with which task number.
