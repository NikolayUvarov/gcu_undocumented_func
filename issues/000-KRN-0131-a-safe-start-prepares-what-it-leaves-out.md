# 000-KRN-0131 — A safe start prepares the services it leaves out, so `svc start` starts them

**Type:** `init` · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made, checked in QEMU for sound and speech; the gate and a radio on a machine left) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-3.12, MC-6.3, MC-6.4

## Problem

The ASTRA branch's hardware audit (H10, [2026-10-15](../issues-audit/2026-10-15-hardware-audit.md)) recorded in `requests-KRN.md` ("Safe-start services need a prepared plan before Platform is dropped"):
- a safe start ([000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md)) skips `bcm_wifi`, `video_gw`, `audio_gw`, `tts` and `rtw89_wifi` before `init` builds their plans;
- `init` drops the platform privilege before READY (MC-3.12);
- so a later `svc start bcm_wifi` has to find its device without that privilege, and fails ("init cannot start it after boot").

## Plan

1. **Prepare instead of skip.** In a safe start, `init` builds a left-out service's plan while it holds the platform privilege:
   - the capabilities are minted and kept, as for a restart;
   - its devices are stopped at once (`DEVICE_STATE` stop), so nothing of them runs;
   - nothing is started.
2. **The station too,** when its radio is one of those left out: `wlan` is prepared over the prepared radio.
3. **Their clients.** `netstack`'s Wi-Fi card and the shell's camera are lent from a prepared station or gateway. Until it is started they answer `ERR_PEER`, as for a service that ended.
4. **`svc start NAME`** then starts it as a restart does: the device is turned on again, and the kept capabilities are granted.

## Acceptance criteria

- [x] A safe start starts no left-out service, and each of them is prepared (`[INIT] NAME LEFT OUT BY THE SAFE START, PREPARED`): the `watchdog` suite.
- [x] After the platform privilege is dropped, `svc start audio_gw` and `svc start tts` start them; `svc start bcm_wifi` answers that its device is missing where there is none: the `watchdog` suite, locally on 4 CPUs.
- [ ] The gate passes.
- [ ] On a machine with a left-out radio (the MacBook Pro's BCM4331), after a safe start, `svc start bcm_wifi` and `svc start wlan` join a network without a reboot.

## Related

[000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md) (the safe start), [213-KRN-0114](213-KRN-0114-realtek-driver-startup.md) (`rtw89_wifi`), docs/profile/README.md (MC-6.4–6.9).
