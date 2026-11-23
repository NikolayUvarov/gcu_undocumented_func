# 213-KRN-0278 — The second screen's lines refill, and a host case runs a plug, an unplug and another plug

**Type:** compositor · **Owner:** `KRN` (the kernel session), for DRV's 213-DRV-0096 · **Priority:** P2 · **Status:** in progress (made, 2026-11-23) · **Blocked by:** — · **Main task:** 213 · **Constitution:** MC-12.2

## Problem

DRV's 213-DRV-0096 will let the second screen come and go within one boot: an unplug gives `screen_gone`, a plug offers the screen again, perhaps at another size. DRV read the compositor and asked for two things (`KRN/2026-11-22T1345Z-DRV-the-second-screen-comes-and-goes-soon-the-compositors-line-cap-and-a-host-case-for-a-second-offer.md` on `internal-dev-messages`).

**The line cap.** Every second-screen line goes through one counter, capped at 24 for the whole boot:
- the offer;
- the screen gone;
- the fit;
- a refusal.

After about a dozen plugs the compositor would fall silent. On a bench run, or on a machine whose monitor is switched daily, the log is the evidence. The remote marks' lines share the same kind of cap.

**No host case.** No host case runs an offer, a gone and an offer of another size. `tests/compositor_mirror_host.rs` covers the placement arithmetic, but `compositor/src/main.rs` is not built on the host.

## Plan

- **`compositor/src/second.rs`, built on the host too.** It holds:
  - `Shape::offered`: the checks of an offer against what the driver lent, and the picture's place for a fit. `Second::new` and `Second::place` use it.
  - `Budget`: 24 lines at once, then one more a minute. A line said after some were not says how many were held back.
- **The second screen's lines and the remote marks' lines** go through a `Budget` each. The watch of the screen in front keeps its cap of 24.
- **`tests/compositor_second_host.rs`:** on the Ryzen's 2560x1600, offer 1920x1080, gone, offer 3440x1440, with the shapes after each; keep and stretch; offers that do not fit what they lent; the budget over a dozen plugs and a minute.
- **Not covered on the host:** the IPC loop, the mapping and the drawing. The hardware run is the maintainer's at the plug, with 213-DRV-0096.

## Acceptance criteria

- [x] The compositor builds for x86 and aarch64.
- [x] The host cases pass, and `tests/compositor_mirror_host.rs` still does.
- [x] The `display` suite passes (the shell's `screen second`, no driver offers a screen in QEMU): QEMU x86, 4 CPUs, locally (2026-11-23), on a tree of main 96d9b7ae00 with 000-KRN-0276, 0277, 213-KRN-0278, 801-NET-0093 and 0094.
- [ ] On the Ryzen with 213-DRV-0096 (DRV, the maintainer at the plug, MC-12.1): each plug and unplug has its line, with the new size.

## Related

213-KRN-0272 (the second screen), 213-KRN-0273 (keep or stretch), DRV's 213-DRV-0096 (plugged and unplugged), APP's 213-APP-0158 (`screen second`).
