# BCM4331 transmit-wait audit — 2026-10-16

**Finding H12:** confirmed control-flow defect, P1 reliability.
**Owner:** DRV (`claude/ASR-DRV`). **Constitution:** MC-6.1, MC-12.1,
MC-12.3. **Roadmap:** D / 550. **Status:** requested from the owner;
not fixed or established as the maintainer's observed packet-loss cause.

## Evidence

Reviewed `bcm_wifi/src/radio.rs::Radio::status` on ASTRA
`ac1854f202ac8ceebb2167c8b3e0da4a539fdd33`, after merging the driver's
original owner branch at `5623c9ba`. This describes that source, not a
claim that the implementation has already passed the gate into main.

The function computes `uptime_ms() + STATUS_MS` (`STATUS_MS = 100`). When
`TX_STATUS0` has its valid bit set, it consumes `TX_STATUS1` and returns
only for a final status with the expected cookie. Otherwise it executes
`continue`, skipping both the deadline check and receive polling below.
A stream of intermediate or wrong-cookie statuses can therefore keep
`send` inside this path beyond its advertised wait. A matching status
arriving after the deadline is also accepted before the time check.

`serve.rs` dispatches `Send` synchronously, and the station waits on that
call. Finite higher-level IPC deadlines do not stop this driver's loop.
This is an exact source finding; no hardware status trace has yet shown
that this condition explains the original MacBook's loss under load.

## Requested correction and acceptance

The [DRV request](../issues/requests-DRV.md#bcm4331-transmit-status-must-check-its-deadline-on-every-path)
asks the owner to preserve the 100 ms contract on every path and to test
the actual production wait logic. Fixtures must include continuously
available intermediate/wrong-cookie statuses, no status, a matching status
before the deadline and one observed after it. The correction must not
reuse a DMA descriptor whose ownership remains uncertain after timeout.
Keep physical MacBook ping/load/recovery acceptance separate from these
software checks. No assertion, time budget or hardware criterion should
be weakened to pass the tests.
