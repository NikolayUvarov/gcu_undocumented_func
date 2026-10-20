# 000-APP-0080 — The Dzen orbit's zero points at the active hour

**Type:** tools / clock display · **Owner:** the tools track (`APP`), at the maintainer's word of 2026-10-20; first worked by the ASTRA auditor-fixer (the maintainer's assignment, 2026-10-15) · **Priority:** P2 · **Status:** open: done on the tools branch and in `fast-test`; the gate and the maintainer's look left · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-12.1, MC-12.3

## Problem

The Ryzen bug notes report the orbit marker pointing bottom-right at 08:20, when the active hour sector is bottom-left. The orbit's one reference tick, the zero of its 100-second turn, stayed toward the first hour corner (bottom right, 00–05) all day.

## The maintainer's correction (2026-10-20)

The first version (ASTRA, `fast-test`, 1c30ffa9, b6b08c86) kept the fixed zero and added a second, colored marker toward the active hour. The maintainer's verdict: "the active hour's tick must coincide with the 100-second counter's tick, that is, zero seconds moves accordingly. It must not be colored. The tick must point at the active hour, and be the exact origin of the 100-second indicator."

## Plan

- **One tick.** The orbit's zero tick points at the active hour's corner, and it is the 100-second turn's zero:
  - the dot is on it at zero seconds of each 100;
  - the nine 10-second ticks of **P** count from it.
- **It moves with the hour.** Every six hours the hour's corner changes, and the whole orbit (ring, zero tick, the nine ticks) is drawn again from the new zero, with nothing left behind.
- **Grey, as before.** The tick keeps the ticks' grey; no colored mark is added.
- **Both faces.** The pixel face and the text face (`--text`) follow the same rule.

## Acceptance criteria

- **Host tests:**
  - the turn starts toward each corner in turn;
  - at 19:35 the pixel face's zero tick points at the top right, is grey, and the dot at zero seconds is on it;
  - at 06:00 the bottom left takes the hour, and the orbit drawn after the change is the one a fresh 06:00 face draws;
  - the text face's zero tick points the same way.
- **The `dzen` suite** (QEMU, 19:35):
  - the reference tick is toward the top right (the yellow hour corner);
  - the nine 10-second ticks are counted from it;
  - nothing in the orbit's ring is colored.
- **On a real machine:** at 08:20 the tick points bottom left, at the hour's green.

## Related

- [089](../issues-done/089-text-clock-faces.done), text clock faces.
- `dzen-clock/src/cycle.rs` (`point_at`), `face.rs` (`Face::hour`), `view.rs`, `text.rs`, `main.rs`; `tests/runtime.rs` (the view), `tests/clock_host.rs`.
- [213](213-amd-ryzen-pc.md), the source of the maintainer's report.

## Progress (2026-10-20)

- **`cycle::point_at(phase, radius, corner)`:** the turn starts toward `corner`.
- **`Face::hour`:** the active hour's corner.
- **The faces.** The pixel face's main loop and `View::cycle` take the corner with the mode, and draw the orbit again when either changes. The text face takes it from the face it draws.
- **ASTRA's colored marker is gone.** Its QEMU check is replaced by the zero tick's place and the ticks counted from it.
- **Host tests:**
  - `tests/runtime.rs`: the dzen view and cycle tests, 12 in all (`zero_seconds_point_at_the_active_hours_corner`, `the_zero_tick_points_at_the_active_hour_and_moves_with_it`);
  - `tests/clock_host.rs`: 16.
- **Left:** the `dzen` suite in the gate; the maintainer's look on a real machine.
