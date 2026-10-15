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

## Normal command/RX extension

The model now continues into the production exclusive `btc_session::Session`
without changing guard registers between stages. Normal channel-12 packets
are consumed only when the actual HCI and stop registers enable command/RX
DMA and the ordinary TX scheduler remains idle. ACK bytes are derived from
the real submitted header, with distinct receipt and completion sequence
positions, and written through the currently programmed RXQ descriptors.
The model releases descriptor ownership only through the production RX
doorbell. The same Region remains alive through independent shutdown.

Four additional cumulative cases cover GEN1/GEN2 RF-page and BTC commands
sharing a single normal sequence, including periodic receipt ACKs; missing
or foreign ACKs despite completed DMA; firmware rejection, truncated ACKs
and delivery through RPQ; and an ACK whose command-DMA index never advances.
Failed sessions reject subsequent commands and borrowed receive permits.
The last case deliberately models a stuck completion report independently
of firmware consumption, because either observation alone is insufficient.

This validates the transport ownership handoff. The modeled firmware emits
ideal synchronous ACKs; it does not execute BTC policy or RF calibration,
and successful command exchange does not establish WLAN operation. These
new cases passed the focused model run recorded below.

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

### Normal Session evidence

Implementation `62df9f0b` built on the own branch without warnings in 1.03
seconds. The three affected binaries were compiled at that exact commit.
After parent-confirmed raw publication `71b3a770`, those unchanged binaries
ran on the own branch on 2026-10-16 MSK (x86-64 WSL2): all ten cumulative
MAC/RX/Session cases passed in 0.11 seconds, and both earlier cold-board cases
passed in 0.01 seconds, with zero failed or ignored. The explicit private-file
GEN1/GEN2 and missing-H2C scenarios also passed, with the same 164 / 164 / 0
packet counts and modeled 12,344 / 12,344 / 407,344 microseconds. Unrelated
imported module tests were filtered; these are not physical-device timings.
Independent review found no blocker in this model extension. Full gate and
physical firmware/RX/BTC behavior remain unvalidated.
