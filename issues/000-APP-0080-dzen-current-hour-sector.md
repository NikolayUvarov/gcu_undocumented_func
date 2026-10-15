# 000-APP-0080 — Mark the current hour sector on the Dzen orbit

**Type:** tools / clock display · **Owner:** ASTRA auditor-fixer (the maintainer's assignment, 2026-10-15) · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Roadmap:** track G · **Constitution:** MC-12.1, MC-12.3

## Problem

The Ryzen bug notes report the orbit marker pointing bottom-right at 08:20,
when the active hour sector is bottom-left. The existing marker is intentionally
the fixed zero reference for the 100-second cycle, as documented in the README;
the clock currently has no separate current-hour-sector marker.

## Plan

Add a colored marker just outside the orbit, toward the active hour corner and
using its color, in both pixel and text faces. Retain the fixed zero reference,
the moving dot, ten-second ticks and existing key behavior. Erase the old marker
when the hour sector changes or the orbit is hidden, including after resize.

## Acceptance criteria

- At 08:20 the new marker points bottom-left; the other three six-hour ranges
  point toward their corresponding hour indicators.
- Sector transitions, midnight and orbit hiding leave no stale marker pixels.
- The 100-second dot and its fixed reference retain their existing meaning.
- Relevant clock host tests pass; hardware visual confirmation is recorded
  separately from host rendering evidence.

## Related

- [089](../issues-done/089-text-clock-faces.done), text clock faces.
- `dzen-clock/src/cycle.rs`, `view.rs`, `text.rs`; `tests/clock_host.rs`.
- [213](213-amd-ryzen-pc.md), the source of the maintainer's report.

## Progress

**2026-10-15, ASTRA:** both renderers now show a colored marker outside the
orbit toward the active hour corner. The pixel renderer erases the preceding
mark on sector changes and when hiding the orbit. The fixed zero reference,
moving dot and ten-second ticks keep their prior meanings.

Validation on ASTRA: `tests/clock_host.rs` passes 16 tests; the three
`dzen_view` tests compiled through `tests/runtime.rs` pass, including a new
rendered-pixel transition/hiding regression. No hardware rendering result is
claimed; the full branch gate remains with the coordinating auditor.
