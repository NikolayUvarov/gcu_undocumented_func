# RTL8852BE cold firmware/MAC/RX model

Task: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md).
This is host model coverage, not physical firmware, RF or WLAN evidence.

`tests/rtw89_pipeline_device.rs` extends the existing cumulative OTP/power/
PCIe/firmware device model. The same cold device, unchanged decoded Board,
command Queue, downloader Ready and retained allocation pass through the
actual PHY-capability query, runtime MAC preparation, RX activation and
Receiving validation. No stage's guard registers are reseeded between
these calls. Device-side reactions follow the commands actually written:
mailbox requests/replies and counter storage, HFC public-page availability,
station initialization and CAM reset completion. They are synchronous model
assumptions; the model does not establish hardware timing or firmware behavior.

The allocation matches native startup: 8 MiB total, a disjoint 128 KiB command
reservation, and the remaining bytes for both RX rings. Both rings are
installed after PCIe finish and before modeled bus mastering. The simulated
RX producer follows the actual programmed descriptor addresses with checked
bounds; it overwrites a consumed DMA buffer at host-index release, so the
fixture checks that C2H bytes already belong to the copied output.

`tests/rtw89_pipeline_mac_host.rs` is registered in `scripts/host_tests.sh`.
Six new cumulative cases cover GEN1/GEN2 success, actual zero/single-NSS
replies, missing capability response, station timeout, failed RX installation
before mastering, and a possibly applied RX-enable error. Successful and
failed attempts independently stop RX, BB/RF and command DMA/WCPU before
modeled PCI mastering is disabled and MAC power is removed; the allocation
outlives those steps. A prior Receiving result is unusable after shutdown.

The synthetic firmware helper is private to the downloader's test module.
It exercises its exact structural parser without weakening the public pinned
SHA-256 policy. The separately fetched private-file fixture remains explicit
and is never bundled with this test, the repository or a disk image.

## Evidence

Own-branch commit `fdb77f1d`, based on consolidated ASTRA `bf5f3df0` plus
`origin/main` at `abb639`, built without warnings in 0.92 seconds. The three
affected harnesses compiled and `bash -n scripts/host_tests.sh` passed.
After raw publication `f566311f`, the following ran on the unchanged own
branch on 2026-10-16 MSK:

- All six new cumulative cases passed (0.11 seconds), none ignored.
- Both prior cold-board/PCIe cases passed (0.01 seconds), none ignored.
- All 18 synthetic downloader cases passed (0.19 seconds). Its one private
  file case was excluded from that ordinary run, then explicitly invoked
  with the separately fetched pinned file and passed.
- The explicit private-file binary passed GEN1 and GEN2 download (164 packets
  each) and the missing-H2C GEN2 case (zero packets). Reported device-model
  durations were 12,344 / 12,344 / 407,344 microseconds, not hardware timings.

The focused filters select the named suites; unrelated imported module cases
were not rerun. Independent read-only review found no blocker. No build or
test ran on `fast-test`; full gate and physical-device acceptance remain open.
