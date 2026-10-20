# RTL8852BE serial H2C host validation — 2026-10-16

Task: [213-DRV-0032](../../issues/213-DRV-0032-rtl8852be-wifi.md).
Contract and firmware trust boundary:
[serial H2C](../../hwdocs/rtl8852be-h2c-runtime.md).

## Exact configuration

- Own branch: `codex/213-DRV-0032-h2c-runtime`.
- Tested source: `c49edca9f2f0989763916fc843561b5d1c5d4e32`.
- Raw publication acknowledged before execution:
  `3c6eaa5648c240670dc29fd59d6b17954a01da2f`.
- Host: Linux x86_64; Rust
  `rustc 1.101.0-nightly (c36f14571 2026-10-01)`.
- Own native release build passed; the inherited unused `initial_phy::Prepared`
  fields warning remained. Each harness was compiled from the exact source with
  `rustc --edition=2021 --test tests/<name>_host.rs -o <binary>` and executed with
  `<binary> --test-threads=1`. No build or test ran on `fast-test`.

## Results

| Harness | Passed | Failed | Existing private fixture ignored | Seconds |
| --- | ---: | ---: | ---: | ---: |
| `rtw89_data_mode` | 288 | 0 | 1 | 9.442 |
| `rtw89_station` | 272 | 0 | 1 | 8.654 |
| `rtw89_btc_session` | 250 | 0 | 1 | 7.731 |
| `rtw89_btc_codec` | 34 | 0 | 1 | 0.416 |
| `rtw89_initial_phy` | 255 | 0 | 1 | 8.009 |
| `rtw89_post_mac` | 250 | 0 | 1 | 7.615 |
| `rtw89_tx` | 250 | 0 | 1 | 7.507 |

Harnesses share imported cases; these counts must not be summed as unique
coverage. The ignored private-firmware fixture is inherited and unchanged; this
run did not execute it. The full machine-local manifest and logs are retained
at `/tmp/rtw89-h2c-runtime-validation/test.json` and the sibling `*-test.log`
files. These paths are local evidence artifacts, not repository inputs.

The seven core wrap cases include 65,538 serial commands, stale host tickets,
receipt/DONE ordering, rejected DONE, missing ACK classes, counter exhaustion
and the unchanged startup limit. The three cumulative cases exercise actual
DataSession activation with busy CH12, 514 commands while Wi-Fi remains queued,
and ACKs arriving while command DMA stays unconsumed after 260 commands.

For every one of the 514 commands, the model checks one pending RX record,
maximum backlog two including the command receipt, and continued producer
progress. It retains the original finite 8192-frame producer, receipt/header
checks, explicit data-pressure counts and subsequent actual modeled TX
completion with `Known(1)`. Ring consumption and ACK completion remain separate
conditions. The shared station suite repeats its exact second-selector-restore
deadline fault after widening the fixture counter.

## Preserved failed executions

1. Source `7e60f2b6f8afe221217fb060664d440f5db20759`, after raw
   `2e267e4bc17342321920c652f8c1b9f9af1d08da`: standalone H2C 20 passed;
   data-mode 286 passed, two failed, one inherited ignore. Both long cases hit
   the host station model's `u8` selector-restoration counter overflow.
   Fixture correction `4d9f9b3626d1f4c7109963ce22965a1313994a72` uses `usize`
   for that counter and its fault ordinal without changing the exact fault.
   Original logs: `/tmp/rtw89-h2c-runtime-first-7e60f2b6/`.
2. Source `4d9f9b3626d1f4c7109963ce22965a1313994a72`, after raw
   `4a4fa150bc430660efe696d302041a9219c36711`: data-mode 287 passed,
   one failed, one inherited ignore. All 514 commands completed, but the
   producer exhausted its finite stream because each released ACK added an
   extra Wi-Fi record. This failed the continuous-traffic assertion and was
   not counted as sustained-traffic success. Correction `c49edca9` replenishes
   only the last published record before release, preserving a nonempty ring
   without ACK-driven backlog growth and adding per-command queue assertions.
   Original logs: `/tmp/rtw89-h2c-runtime-second-4d9f9b36/`.

Both corrections change host fixtures only; the production H2C/runtime code
was unchanged. Failed runs remain failed and are not replaced by the corrected
results. Independent source reviews covered the generation/ACK contract,
actual DMA completion and bounded producer/release behavior before execution.

## Limits

This is evidence from the stated host models, not physical RTL8852BE traffic,
native service availability, a full project gate or an RF emissions
measurement. The device model uses its documented deterministic hardware
completion behavior. Software ticket generation rejects stale host handles;
wire ACKs still contain an eight-bit sequence, so an arbitrary ancient ACK
replayed after a full cycle is indistinguishable. Firmware ordered requested
completion remains part of the trusted protocol. Rejection is terminal and
does not establish DMA quiescence; retained cleanup remains necessary.

The issue stays open for the remaining native, connected-role policy and
physical acceptance requirements. The frozen aggregate candidate is validated
separately; this own-branch result is not transferred to another configuration.
