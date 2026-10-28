# RTL8852BE BTC 105 receive-envelope evidence

Task: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md).
This decoder is an RX prerequisite, not a firmware-policy acknowledgement,
RFK arbitration grant, initialized BTC state or working WLAN endpoint.

The implementation follows Linux v6.19 commit
`05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`, `coex.c`, `core.h` and `fw.h`,
for the pinned RTL8852B firmware 0.29.29.15 / BTC profile 105. It validates
the complete bounded report stream before exposing borrowed records;
TDMA-v3 preserves all eight policy bytes, slots-v1 preserves all 18 slots
and the update bitmap, and BT-version-v1 preserves the complete 32-bit
fields. Unknown records and BTINFO/scoreboard/overflow/loopback events
remain explicit. The session owner must establish their provenance and
process changing BT state before making a policy or RFK decision.

## Host evidence, 2026-10-16 MSK

Source commit `96941305`, host-gate registration `3b0ca3d4`, on the agent's
`codex/213-DRV-0032-rtl8852be` branch. The native release build passed without
warnings (0.76 seconds for the source commit; the registration-only rebuild
reported 0.00 seconds), and `bash -n scripts/host_tests.sh` passed.
After raw publication in `origin/fast-test` at `539b588e`, the unchanged
own-branch fixture was compiled and run with:

```sh
rustc --edition=2021 --test tests/rtw89_btc_reports_host.rs -o /tmp/rtw89-btc-reports-3b0ca3d4
/tmp/rtw89-btc-reports-3b0ca3d4
```

All seven cases passed, none failed, ignored or filtered; the harness
reported 0.00 seconds. They cover packed/misaligned values, exact profile
mapping, typed lengths/versions/counts, malformed trailing records and
truncations, the 1800-byte bound, empty-record progress, and visible events.
Independent read-only source review found no blocking defect.

No test or build ran on `fast-test`. These are host decoder results only;
the complete gate, real firmware reports, session policy application and
physical-device operation are not established by this evidence.
